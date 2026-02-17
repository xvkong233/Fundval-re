export type TimeRange = "1W" | "1M" | "3M" | "6M" | "1Y" | "ALL";

function toYmd(d: Date): string {
  const year = d.getUTCFullYear();
  const month = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function getDateRange(range: TimeRange, now: Date = new Date()) {
  const end = new Date(now.getTime());
  const start = new Date(now.getTime());

  switch (range) {
    case "1W":
      start.setUTCDate(start.getUTCDate() - 7);
      break;
    case "1M":
      start.setUTCMonth(start.getUTCMonth() - 1);
      break;
    case "3M":
      start.setUTCMonth(start.getUTCMonth() - 3);
      break;
    case "6M":
      start.setUTCMonth(start.getUTCMonth() - 6);
      break;
    case "1Y":
      start.setUTCFullYear(start.getUTCFullYear() - 1);
      break;
    case "ALL":
      start.setUTCFullYear(start.getUTCFullYear() - 10);
      break;
  }

  return { startDate: toYmd(start), endDate: toYmd(end) };
}

