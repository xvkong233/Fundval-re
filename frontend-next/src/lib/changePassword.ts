type AxiosLikeError = {
  response?: {
    status?: number;
    data?: unknown;
  };
  message?: string;
};

const getBackendError = (data: unknown): string | undefined => {
  if (!data || typeof data !== "object") return undefined;
  const maybe = (data as any).error;
  return typeof maybe === "string" && maybe.trim() ? maybe : undefined;
};

export const getChangePasswordErrorMessage = (error: unknown): string => {
  const err = error as AxiosLikeError;
  const status = err?.response?.status;
  const backend = getBackendError(err?.response?.data);

  if (status === 401) {
    return "登录状态已失效，请重新登录后再试。";
  }

  if (!err?.response) {
    return "无法连接到服务器。请检查后端服务是否运行，以及 API_PROXY_TARGET 配置。";
  }

  return backend ?? err?.message ?? "修改密码失败";
};

