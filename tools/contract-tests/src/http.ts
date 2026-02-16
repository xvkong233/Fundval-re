import type { JsonValue } from "./diff.js";

export type HttpResponse = {
  status: number;
  json: JsonValue;
};

export async function getJson(url: string): Promise<HttpResponse> {
  const res = await fetch(url, {
    method: "GET",
    headers: { Accept: "application/json" }
  });
  const text = await res.text();
  let json: JsonValue;
  try {
    json = JSON.parse(text) as JsonValue;
  } catch {
    throw new Error(`非 JSON 响应: ${url} status=${res.status} body=${text.slice(0, 200)}`);
  }
  return { status: res.status, json };
}

