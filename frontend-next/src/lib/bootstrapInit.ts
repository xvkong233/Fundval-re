"use client";

export type BootstrapInitErrorKind = "already_initialized" | "invalid_key" | "network" | "unknown";

export type BootstrapInitErrorInfo = {
  kind: BootstrapInitErrorKind;
  message: string;
  status?: number;
};

export const maskBootstrapKey = (key: string): string => {
  const trimmed = key.trim();
  if (!trimmed) return "";
  if (trimmed.length <= 8) return "****";
  return `${trimmed.slice(0, 4)}…${trimmed.slice(-4)}`;
};

type AxiosLikeError = {
  response?: {
    status?: number;
    data?: unknown;
  };
  message?: string;
};

const getBackendErrorMessage = (data: unknown): string | undefined => {
  if (!data || typeof data !== "object") return undefined;
  const maybe = (data as any).error;
  return typeof maybe === "string" && maybe.trim() ? maybe : undefined;
};

export const getBootstrapInitError = (error: unknown): BootstrapInitErrorInfo => {
  const err = error as AxiosLikeError;
  const status = err?.response?.status;
  const backendMessage = getBackendErrorMessage(err?.response?.data);

  if (status === 410) {
    return {
      kind: "already_initialized",
      status,
      message: backendMessage ?? "系统已初始化，接口失效",
    };
  }

  if (status === 400) {
    return {
      kind: "invalid_key",
      status,
      message: backendMessage ?? "密钥无效",
    };
  }

  if (!err?.response) {
    return {
      kind: "network",
      message:
        "无法连接到服务器。请检查后端服务是否运行，以及前端环境变量 API_PROXY_TARGET 是否指向正确的后端地址。",
    };
  }

  return {
    kind: "unknown",
    status,
    message: backendMessage ?? err?.message ?? "操作失败",
  };
};
