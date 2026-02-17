"use client";

import api, { publicApi } from "./http";

export const healthCheck = () => api.get("/health/");

export const verifyBootstrapKey = (key: string) =>
  publicApi.post("/admin/bootstrap/verify", { bootstrap_key: key });

export const initializeSystem = (data: {
  bootstrap_key: string;
  admin_username: string;
  admin_password: string;
  allow_register: boolean;
}) => publicApi.post("/admin/bootstrap/initialize", data);

export const login = (username: string, password: string) =>
  publicApi.post("/auth/login", { username, password });

export const register = (username: string, password: string, passwordConfirm: string) =>
  publicApi.post("/users/register/", { username, password, password_confirm: passwordConfirm });

export const refreshToken = (refreshTokenValue: string) =>
  publicApi.post("/auth/refresh", { refresh_token: refreshTokenValue });

export const getCurrentUser = () => api.get("/auth/me");

