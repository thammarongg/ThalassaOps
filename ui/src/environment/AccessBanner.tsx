import { useState } from "react";
import type { CloudAccessState } from "../../contracts/ipc";
import { useTranslation } from "../i18n";

export function AccessBanner({
  access,
  remedy,
  loading = false
}: {
  access?: CloudAccessState;
  remedy?: string;
  loading?: boolean;
}) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  if (loading || !access) {
    return (
      <p className="environment-access-loading" role="status">
        {t("environment.accessChecking")}
      </p>
    );
  }

  if (access === "confirmed") return null;

  const copyableRemedy = remedy?.trim() || t("environment.accessCheckRemedy");
  const copyRemedy = async () => {
    if (!navigator.clipboard) return;
    try {
      await navigator.clipboard.writeText(copyableRemedy);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  return (
    <aside className={`environment-access environment-access--${access}`} role="alert">
      <div className="environment-access__copy">
        <p className="environment-access__eyebrow">{t("environment.accessLabel")}</p>
        <h3>{t(`environment.access.${access}`)}</h3>
        <p>{t("environment.remedyLabel")}</p>
        <code>{copyableRemedy}</code>
      </div>
      <div className="environment-access__actions">
        <button type="button" onClick={() => void copyRemedy()}>
          {t("environment.copyRemedy")}
        </button>
        {copied && <span role="status">{t("environment.copiedRemedy")}</span>}
      </div>
    </aside>
  );
}
