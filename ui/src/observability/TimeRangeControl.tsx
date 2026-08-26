import { useTranslation } from "../i18n";
import type { TimeContext } from "./timeContext";

const pad = (value: number) => String(value).padStart(2, "0");

const toLocalInputValue = (value: string) => {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return [
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
    `${pad(date.getHours())}:${pad(date.getMinutes())}`
  ].join("T");
};

const toIsoValue = (value: string) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
};

export function TimeRangeControl({
  timeContext,
  onChange
}: {
  timeContext: TimeContext;
  onChange: (context: TimeContext) => void;
}) {
  const { t } = useTranslation();
  const update = (field: "start" | "end", value: string) => {
    const isoValue = toIsoValue(value);
    if (!isoValue) return;
    onChange({ ...timeContext, [field]: isoValue, source: "manual" });
  };

  return (
    <fieldset className="time-range-control">
      <legend>{t("observability.timeRange")}</legend>
      <label htmlFor="observability-time-range-start">
        {t("observability.startTime")}
      </label>
      <input
        id="observability-time-range-start"
        type="datetime-local"
        value={toLocalInputValue(timeContext.start)}
        onChange={(event) => update("start", event.target.value)}
      />
      <label htmlFor="observability-time-range-end">{t("observability.endTime")}</label>
      <input
        id="observability-time-range-end"
        type="datetime-local"
        value={toLocalInputValue(timeContext.end)}
        onChange={(event) => update("end", event.target.value)}
      />
    </fieldset>
  );
}
