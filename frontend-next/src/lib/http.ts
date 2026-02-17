"use client";

import axios from "axios";
import { getToken, logout, setToken } from "./auth";

const DEFAULT_TIMEOUT_MS = 10_000;

const api = axios.create({
  baseURL: "/api",
  timeout: DEFAULT_TIMEOUT_MS,
  headers: { "Content-Type": "application/json" },
});

export const publicApi = axios.create({
  baseURL: "/api",
  timeout: DEFAULT_TIMEOUT_MS,
  headers: { "Content-Type": "application/json" },
});

api.interceptors.request.use(
  (config) => {
    const { accessToken } = getToken();
    if (accessToken) {
      config.headers = config.headers ?? {};
      (config.headers as any).Authorization = `Bearer ${accessToken}`;
    }
    return config;
  },
  (error) => Promise.reject(error)
);

api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest: any = error.config;

    if (
      error.response?.status === 401 &&
      originalRequest &&
      !originalRequest._retry &&
      typeof originalRequest.url === "string" &&
      !originalRequest.url.includes("/auth/refresh")
    ) {
      originalRequest._retry = true;

      try {
        const { refreshToken } = getToken();
        if (!refreshToken) throw new Error("missing refresh_token");

        const resp = await publicApi.post("/auth/refresh", { refresh_token: refreshToken });
        const accessToken = resp.data?.access_token as string | undefined;
        if (!accessToken) throw new Error("refresh response missing access_token");

        setToken(accessToken, refreshToken);
        originalRequest.headers = originalRequest.headers ?? {};
        originalRequest.headers.Authorization = `Bearer ${accessToken}`;
        return api(originalRequest);
      } catch (refreshError) {
        logout();
        window.location.href = "/login";
        return Promise.reject(refreshError);
      }
    }

    return Promise.reject(error);
  }
);

export default api;

