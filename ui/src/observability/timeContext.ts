export type TimeContext = {
  start: string;
  end: string;
  source: "alert" | "manual";
};

type AlertTimeFields = {
  starts_at: string;
  state: string;
  ends_at?: string | null;
};

export function timeContextFromAlert(alert: AlertTimeFields, now: string | Date): TimeContext {
  const nowValue = typeof now === "string" ? now : now.toISOString();
  return {
    start: alert.starts_at,
    end: alert.state === "resolved" && alert.ends_at ? alert.ends_at : nowValue,
    source: "alert"
  };
}
