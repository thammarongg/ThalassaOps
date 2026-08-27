use chrono::DateTime;
use regex::Regex;
use reqwest::Url;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thalassaops::cloud::{
    AwsCredentialProvider, AzureCredentialProvider, CloudClient, CloudTextResponse,
    GcpCredentialProvider,
};

const DEFAULT_OUTPUT_DIR: &str = "fixture-captures";

struct Redactor {
    exact: Vec<(String, String)>,
    replacements: HashMap<(String, String), String>,
    counters: HashMap<String, usize>,
}

impl Redactor {
    fn new() -> Self {
        Self {
            exact: Vec::new(),
            replacements: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    fn add_exact(&mut self, original: &str, placeholder: &str) {
        if !original.is_empty() {
            self.exact
                .push((original.to_owned(), placeholder.to_owned()));
        }
    }

    fn placeholder(&mut self, category: &str, original: &str) -> String {
        let key = (category.to_owned(), original.to_owned());
        if let Some(existing) = self.replacements.get(&key) {
            return existing.clone();
        }
        let counter = self.counters.entry(category.to_owned()).or_insert(0);
        *counter += 1;
        let placeholder = if *counter == 1 {
            format!("<{category}>")
        } else {
            format!("<{category}_{counter}>")
        };
        self.replacements.insert(key, placeholder.clone());
        placeholder
    }

    fn redact(&mut self, body: &str, content_type: Option<&str>) -> String {
        // Substitute selectors before generic patterns so a project such as
        // `doca-262908` cannot be split into a visible prefix and a redacted
        // number. Azure resource IDs are then matched with the subscription
        // placeholder as well, preserving the whole-resource redaction.
        let mut replaced = body.to_owned();
        for (original, placeholder) in &self.exact {
            replaced = replaced.replace(original, placeholder);
        }

        if content_type.is_some_and(|value| value.to_ascii_lowercase().contains("json"))
            || replaced.trim_start().starts_with('{')
            || replaced.trim_start().starts_with('[')
        {
            if let Ok(mut value) = serde_json::from_str::<Value>(&replaced) {
                self.redact_json_value(None, &mut value);
                if let Ok(formatted) = serde_json::to_string_pretty(&value) {
                    return formatted;
                }
            }
        }

        replaced = self.redact_text(&replaced);
        for (original, placeholder) in &self.exact {
            replaced = replaced.replace(original, placeholder);
        }

        if content_type.is_some_and(|value| value.to_ascii_lowercase().contains("xml")) {
            self.xml_safe_placeholders(&replaced)
        } else {
            replaced
        }
    }

    fn redact_json_value(&mut self, key: Option<&str>, value: &mut Value) {
        self.redact_json_value_in_context(key, value, false);
    }

    fn redact_json_value_in_context(
        &mut self,
        key: Option<&str>,
        value: &mut Value,
        sensitive_context: bool,
    ) {
        let sensitive_context = sensitive_context || key.is_some_and(is_sensitive_key);
        match value {
            Value::Object(fields) => {
                for (field, value) in fields {
                    self.redact_json_value_in_context(Some(field), value, sensitive_context);
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.redact_json_value_in_context(key, value, sensitive_context);
                }
            }
            Value::String(text) => {
                let current = std::mem::take(text);
                *text = if sensitive_context {
                    self.placeholder("SENSITIVE_DATA", &current)
                } else if key.is_some_and(|field| field.eq_ignore_ascii_case("id"))
                    && current.len() >= 8
                    && current
                        .chars()
                        .all(|character| character.is_ascii_hexdigit())
                {
                    self.placeholder("CLOUD_RESOURCE_ID", &current)
                } else if is_rfc3339_timestamp(&current) {
                    current
                } else {
                    self.redact_text(&current)
                };
            }
            Value::Number(number)
                if sensitive_context
                    || key.is_some_and(|field| {
                        let normalized = field.to_ascii_lowercase();
                        normalized.contains("account")
                            || normalized.contains("project")
                            || normalized.contains("subscription")
                            || normalized == "number"
                    }) =>
            {
                let original = number.to_string();
                *value = Value::String(self.placeholder("ACCOUNT_OR_PROJECT_NUMBER", &original));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    fn redact_text(&mut self, body: &str) -> String {
        let mut replaced = body.to_owned();
        replaced = self.replace_matches(
            &replaced,
            r"(?s)-----BEGIN [^-]+-----.*?-----END [^-]+-----",
            "PEM_DATA",
        );
        replaced = self.replace_matches(
            &replaced,
            r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+",
            "BEARER_TOKEN",
        );
        replaced = self.replace_matches(&replaced, r#"arn:aws:[^\s\"'<>]+"#, "AWS_ARN");
        replaced = self.replace_matches(
            &replaced,
            r#"(?i)/subscriptions/[0-9a-f-]+/resourceGroups/[^\s\"'<>]+"#,
            "AZURE_RESOURCE_ID",
        );
        replaced = self.replace_matches(
            &replaced,
            r#"(?i)/subscriptions/(?:[0-9a-f-]+|<AZURE_SUBSCRIPTION_ID>)/resourceGroups/[^\s\"'<>]+"#,
            "AZURE_RESOURCE_ID",
        );
        replaced = self.replace_matches(
            &replaced,
            r"\b(?:25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])(?:\.(?:25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])){3}\b",
            "IP_ADDRESS",
        );
        replaced = self.replace_matches(
            &replaced,
            r"\b[0-9A-Fa-f]{1,4}(?::[0-9A-Fa-f]{1,4}){2,7}\b",
            "IP_ADDRESS",
        );
        replaced = self.replace_matches(&replaced, r"\b\d{6,12}\b", "ACCOUNT_OR_PROJECT_NUMBER");
        replaced = self.replace_matches(
            &replaced,
            r"\b(?:i|vpc|subnet|sg|eni|vol|igw|rtb|nat|r|ami|eipalloc|eipassoc)-[0-9a-z-]+\b",
            "CLOUD_RESOURCE_ID",
        );
        replaced = self.replace_matches(&replaced, r"\b[0-9A-Fa-f]{32,}\b", "CLOUD_RESOURCE_ID");
        replaced = self.replace_matches(
            &replaced,
            r"\b[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\b",
            "CLOUD_RESOURCE_ID",
        );
        replaced = self.replace_matches(
            &replaced,
            r"\bssh-(?:rsa|ed25519|ecdsa)\s+[A-Za-z0-9+/=]+(?:\s+[^\s]+)?",
            "SSH_KEY",
        );
        replaced = self.replace_xml_sensitive_values(&replaced);
        replaced = self.replace_xml_sensitive_attributes(&replaced);
        replaced = self.replace_matches(
            &replaced,
            r"(?i)\b(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+(?:amazonaws\.com|googleapis\.com|googleusercontent\.com|azure\.com|windows\.net)\b",
            "DNS_NAME",
        );
        self.replace_matches(
            &replaced,
            r"\b(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}\b",
            "DNS_NAME",
        )
    }

    fn replace_matches(&mut self, input: &str, expression: &str, category: &str) -> String {
        let regex = Regex::new(expression).expect("capture redaction regex");
        let mut output = String::with_capacity(input.len());
        let mut cursor = 0;
        for matched in regex.find_iter(input) {
            let value = matched.as_str();
            if category == "IP_ADDRESS"
                && value.contains('.')
                && value
                    .split('.')
                    .any(|octet| octet.parse::<u16>().unwrap_or(256) > 255)
            {
                continue;
            }
            output.push_str(&input[cursor..matched.start()]);
            output.push_str(&self.placeholder(category, value));
            cursor = matched.end();
        }
        output.push_str(&input[cursor..]);
        output
    }

    fn replace_xml_sensitive_values(&mut self, input: &str) -> String {
        let regex = Regex::new(
            r#"(?is)(<(?:authorization|access[_-]?token|bearer[_-]?token|refresh[_-]?token|client[_-]?token|token|certificate(?:Data)?|(?:access|secret|public|private)[_-]?key(?:Id|Data)?|ssh[_-]?(?:public|private)[_-]?key)[^>]*>)(.*?)(</(?:authorization|access[_-]?token|bearer[_-]?token|refresh[_-]?token|client[_-]?token|token|certificate(?:Data)?|(?:access|secret|public|private)[_-]?key(?:Id|Data)?|ssh[_-]?(?:public|private)[_-]?key)>)"#,
        )
        .expect("capture XML redaction regex");
        let mut output = String::with_capacity(input.len());
        let mut cursor = 0;
        for captures in regex.captures_iter(input) {
            let whole = captures.get(0).expect("whole XML match");
            let opening = captures.get(1).expect("opening XML match").as_str();
            let closing = captures.get(3).expect("closing XML match").as_str();
            output.push_str(&input[cursor..whole.start()]);
            output.push_str(opening);
            output.push_str(&self.placeholder("SENSITIVE_DATA", captures.get(2).unwrap().as_str()));
            output.push_str(closing);
            cursor = whole.end();
        }
        output.push_str(&input[cursor..]);
        output
    }

    fn replace_xml_sensitive_attributes(&mut self, input: &str) -> String {
        let regex = Regex::new(
            r#"(?i)((?:authorization|access_token|token|certificate|publicKey|privateKey)\s*=\s*[\"'])(.*?)([\"'])"#,
        )
        .expect("capture XML attribute redaction regex");
        let mut output = String::with_capacity(input.len());
        let mut cursor = 0;
        for captures in regex.captures_iter(input) {
            let whole = captures.get(0).expect("whole XML attribute match");
            let prefix = captures.get(1).expect("XML attribute prefix").as_str();
            let suffix = captures.get(3).expect("XML attribute suffix").as_str();
            output.push_str(&input[cursor..whole.start()]);
            output.push_str(prefix);
            output.push_str(&self.placeholder("SENSITIVE_DATA", captures.get(2).unwrap().as_str()));
            output.push_str(suffix);
            cursor = whole.end();
        }
        output.push_str(&input[cursor..]);
        output
    }

    fn xml_safe_placeholders(&self, input: &str) -> String {
        let regex = Regex::new(r"<(?:[A-Z][A-Z0-9_]*)(?:_[0-9]+)?>")
            .expect("capture XML placeholder regex");
        regex
            .replace_all(input, |capture: &regex::Captures<'_>| {
                format!("REDACTED_{}", &capture[0][1..capture[0].len() - 1])
            })
            .into_owned()
    }
}

fn is_rfc3339_timestamp(value: &str) -> bool {
    DateTime::parse_from_rfc3339(value).is_ok()
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "authorization",
        "token",
        "secret",
        "password",
        "credential",
        "certificate",
        "privatekey",
        "publickey",
        "accesskey",
        "signature",
        "ssh",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("required environment variable {name} is not set"))
}

fn parse_url(value: String) -> Result<Url, String> {
    Url::parse(&value).map_err(|_| "capture URL could not be constructed".into())
}

fn output_path(root: &Path, provider: &str, operation: &str, extension: &str) -> PathBuf {
    root.join(provider).join(format!("{operation}.{extension}"))
}

async fn capture_call(
    client: &CloudClient,
    operation: &str,
    url: Url,
    path: &Path,
    redactor: &mut Redactor,
) -> Result<(), String> {
    let response: CloudTextResponse = client
        .get_text_with_status(url)
        .await
        .map_err(|error| error.to_string())?;
    let body_bytes = response.body.len();
    let redacted = redactor.redact(&response.body, response.content_type.as_deref());
    fs::write(path, redacted).map_err(|_| format!("could not write capture for {operation}"))?;
    println!(
        "{operation} status={} content_type={} body_bytes={body_bytes}",
        response.status,
        response.content_type.as_deref().unwrap_or("<absent>")
    );
    Ok(())
}

async fn capture_aws(root: &Path) -> Result<(), String> {
    let profile = required_env("THALASSAOPS_CAPTURE_AWS_PROFILE")?;
    let region = required_env("THALASSAOPS_CAPTURE_AWS_REGION")?;
    let cluster = required_env("THALASSAOPS_CAPTURE_AWS_EKS_CLUSTER")?;
    fs::create_dir_all(root.join("aws")).map_err(|_| "could not create AWS capture directory")?;

    let client = CloudClient::new(Arc::new(AwsCredentialProvider::new(
        profile.clone(),
        region.clone(),
    )))
    .map_err(|error| error.to_string())?;
    let mut redactor = Redactor::new();
    redactor.add_exact(&profile, "<AWS_PROFILE>");
    redactor.add_exact(&cluster, "<AWS_CLUSTER_NAME>");

    capture_call(
        &client,
        "aws_eks_list_clusters",
        parse_url(format!(
            "https://eks.{region}.amazonaws.com/clusters?maxResults=1"
        ))?,
        &output_path(root, "aws", "aws_eks_list_clusters", "json"),
        &mut redactor,
    )
    .await?;
    capture_call(
        &client,
        "aws_eks_describe_cluster",
        parse_url(format!(
            "https://eks.{region}.amazonaws.com/clusters/{cluster}"
        ))?,
        &output_path(root, "aws", "aws_eks_describe_cluster", "json"),
        &mut redactor,
    )
    .await?;
    capture_call(
        &client,
        "aws_ec2_describe_instances",
        parse_url(format!(
            "https://ec2.{region}.amazonaws.com/?Action=DescribeInstances&Version=2016-11-15&MaxResults=5"
        ))?,
        &output_path(root, "aws", "aws_ec2_describe_instances", "xml"),
        &mut redactor,
    )
    .await
}

async fn capture_azure(root: &Path) -> Result<(), String> {
    let subscription = required_env("THALASSAOPS_CAPTURE_AZURE_SUBSCRIPTION_ID")?;
    let tenant = required_env("THALASSAOPS_CAPTURE_AZURE_TENANT_ID")?;
    fs::create_dir_all(root.join("azure"))
        .map_err(|_| "could not create Azure capture directory")?;

    let credential =
        AzureCredentialProvider::new(tenant.clone()).map_err(|error| error.to_string())?;
    let client = CloudClient::new(Arc::new(credential)).map_err(|error| error.to_string())?;
    let mut redactor = Redactor::new();
    redactor.add_exact(&subscription, "<AZURE_SUBSCRIPTION_ID>");
    redactor.add_exact(&tenant, "<AZURE_TENANT_ID>");

    capture_call(
        &client,
        "azure_aks_managed_clusters",
        parse_url(format!(
            "https://management.azure.com/subscriptions/{subscription}/providers/Microsoft.ContainerService/managedClusters?api-version=2026-05-01"
        ))?,
        &output_path(root, "azure", "azure_aks_managed_clusters", "json"),
        &mut redactor,
    )
    .await?;
    capture_call(
        &client,
        "azure_compute_virtual_machines_status_only",
        parse_url(format!(
            "https://management.azure.com/subscriptions/{subscription}/providers/Microsoft.Compute/virtualMachines?api-version=2026-03-01&statusOnly=true"
        ))?,
        &output_path(
            root,
            "azure",
            "azure_compute_virtual_machines_status_only",
            "json",
        ),
        &mut redactor,
    )
    .await
}

async fn capture_gcp(root: &Path) -> Result<(), String> {
    let project = required_env("THALASSAOPS_CAPTURE_GCP_PROJECT_ID")?;
    fs::create_dir_all(root.join("gcp")).map_err(|_| "could not create GCP capture directory")?;

    let client = CloudClient::new(Arc::new(GcpCredentialProvider::new()))
        .map_err(|error| error.to_string())?;
    let mut redactor = Redactor::new();
    redactor.add_exact(&project, "<GCP_PROJECT_ID>");

    capture_call(
        &client,
        "gcp_gke_list_clusters",
        parse_url(format!(
            "https://container.googleapis.com/v1/projects/{project}/locations/-/clusters"
        ))?,
        &output_path(root, "gcp", "gcp_gke_list_clusters", "json"),
        &mut redactor,
    )
    .await?;
    capture_call(
        &client,
        "gcp_compute_aggregated_instances",
        parse_url(format!(
            "https://compute.googleapis.com/compute/v1/projects/{project}/aggregated/instances?maxResults=1&returnPartialSuccess=true"
        ))?,
        &output_path(root, "gcp", "gcp_compute_aggregated_instances", "json"),
        &mut redactor,
    )
    .await
}

async fn run(args: &[String]) -> Result<(), String> {
    let requested = args.get(1).map(String::as_str).unwrap_or("all");
    let providers = if requested == "all" {
        vec!["aws", "azure", "gcp"]
    } else {
        requested.split(',').collect::<Vec<_>>()
    };
    if providers.is_empty()
        || providers
            .iter()
            .any(|provider| !matches!(*provider, "aws" | "azure" | "gcp"))
    {
        return Err("provider must be one or more of: aws, azure, gcp (comma-separated)".into());
    }
    let output = PathBuf::from(
        env::var("THALASSAOPS_CAPTURE_OUTPUT_DIR").unwrap_or_else(|_| DEFAULT_OUTPUT_DIR.into()),
    );
    fs::create_dir_all(&output).map_err(|_| "could not create capture output directory")?;

    let mut failures = 0;
    for provider in providers {
        let result = match provider {
            "aws" => capture_aws(&output).await,
            "azure" => capture_azure(&output).await,
            "gcp" => capture_gcp(&output).await,
            _ => unreachable!("provider checked above"),
        };
        if let Err(error) = result {
            failures += 1;
            eprintln!("{provider}: {error}");
        }
    }
    if failures == 0 {
        Ok(())
    } else {
        Err(format!("{failures} provider capture(s) failed"))
    }
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("create Tokio runtime");
    if let Err(error) = runtime.block_on(run(&env::args().collect::<Vec<_>>())) {
        eprintln!("capture failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_preserves_shape_and_reuses_stable_placeholders() {
        let mut redactor = Redactor::new();
        redactor.add_exact("fixture-project", "<GCP_PROJECT_ID>");
        let first = redactor.redact(
            r#"{"project":"fixture-project","address":"10.1.2.3","token":"Bearer abc"}"#,
            Some("application/json"),
        );
        let second = redactor.redact(
            r#"{"project":"fixture-project","address":"10.1.2.3","token":"Bearer xyz"}"#,
            Some("application/json"),
        );

        assert!(first.contains("<GCP_PROJECT_ID>"));
        assert!(first.contains("<IP_ADDRESS>"));
        assert!(first.contains("<SENSITIVE_DATA>"));
        assert!(!first.contains("fixture-project"));
        assert!(!first.contains("10.1.2.3"));
        assert!(!first.contains("Bearer abc"));
        assert!(second.contains("<IP_ADDRESS>"));
        assert!(second.contains("<SENSITIVE_DATA_2>"));
        assert!(!second.contains("Bearer xyz"));

        let namespace = redactor.redact(
            r#"{"type":"Microsoft.Compute/virtualMachines","endpoint":"vm.example.com"}"#,
            Some("application/json"),
        );
        assert!(namespace.contains("Microsoft.Compute/virtualMachines"));
        assert!(namespace.contains("<DNS_NAME>"));

        redactor.add_exact("doca-262908", "<GCP_PROJECT_ID>");
        let project_with_number = redactor.redact(
            r#"{"project":"doca-262908","certificateAuthority":{"data":"secret"}}"#,
            Some("application/json"),
        );
        assert!(!project_with_number.contains("doca-262908"));
        assert!(project_with_number.contains("<SENSITIVE_DATA"));
        assert!(!project_with_number.contains("secret"));
    }

    #[test]
    fn xml_redaction_keeps_tags_and_replaces_sensitive_content() {
        let mut redactor = Redactor::new();
        let result = redactor.redact(
            "<Response><Token>Bearer abc</Token><AccessToken>opaque-access-token</AccessToken><Header authorization=\"opaque-token\" /><Address>192.168.1.5</Address></Response>",
            Some("text/xml"),
        );

        assert!(
            result.contains("<Token>REDACTED_SENSITIVE_DATA</Token>"),
            "redacted XML: {result}"
        );
        assert!(result.contains("<AccessToken>REDACTED_SENSITIVE_DATA_2</AccessToken>"));
        assert!(result.contains("<Address>REDACTED_IP_ADDRESS</Address>"));
        assert!(result.contains("authorization=\"REDACTED_SENSITIVE_DATA_3\""));
        assert!(!result.contains("Bearer abc"));
        assert!(!result.contains("opaque-access-token"));
        assert!(!result.contains("opaque-token"));
        assert!(!result.contains("192.168.1.5"));
    }

    #[test]
    fn azure_resource_ids_are_redacted_as_a_single_value() {
        let mut redactor = Redactor::new();
        redactor.add_exact(
            "12345678-1234-1234-1234-123456789012",
            "<AZURE_SUBSCRIPTION_ID>",
        );
        let result = redactor.redact(
            "{\"id\":\"/subscriptions/12345678-1234-1234-1234-123456789012/resourceGroups/fixture/providers/Microsoft.Compute/virtualMachines/vm-1\"}",
            Some("application/json"),
        );

        assert!(result.contains("<AZURE_RESOURCE_ID>"));
        assert!(!result.contains("fixture"));
        assert!(!result.contains("vm-1"));
    }

    #[test]
    fn json_redaction_preserves_timestamps_and_document_structure() {
        let mut redactor = Redactor::new();
        let result = redactor.redact(
            r#"{"properties":{"instanceView":{"statuses":[{"code":"ProvisioningState/succeeded","time":"2026-08-27T17:21:37.123456+00:00"},{"code":"PowerState/running"}]},"tail":"retained"},"keyData":"ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC fixture"}"#,
            Some("application/json; charset=utf-8"),
        );

        let parsed: Value = serde_json::from_str(&result).expect("redacted JSON remains valid");
        assert_eq!(
            parsed["properties"]["instanceView"]["statuses"][0]["time"],
            "2026-08-27T17:21:37.123456+00:00"
        );
        assert_eq!(
            parsed["properties"]["instanceView"]["statuses"][1]["code"],
            "PowerState/running"
        );
        assert_eq!(parsed["properties"]["tail"], "retained");
        assert_eq!(parsed["keyData"], "<SSH_KEY>");
    }
}
