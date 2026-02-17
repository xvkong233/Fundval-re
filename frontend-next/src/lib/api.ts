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

// funds
export const listFunds = (params: {
  page?: number;
  page_size?: number;
  search?: string;
  fund_type?: string;
}) => api.get("/funds/", { params });

export const getFundDetail = (fundCode: string) => api.get(`/funds/${encodeURIComponent(fundCode)}/`);

export const getFundEstimate = (fundCode: string, source?: string) =>
  api.get(`/funds/${encodeURIComponent(fundCode)}/estimate/`, { params: source ? { source } : {} });

export const batchEstimate = (fundCodes: string[]) =>
  api.post("/funds/batch_estimate/", { fund_codes: fundCodes });

export const batchUpdateNav = (fundCodes: string[]) =>
  api.post("/funds/batch_update_nav/", { fund_codes: fundCodes });

// nav history
export const listNavHistory = (fundCode: string, params: Record<string, any> = {}) =>
  api.get("/nav-history/", { params: { fund_code: fundCode, ...params } });

export const syncNavHistory = (fundCodes: string[], startDate: string, endDate: string) =>
  api.post("/nav-history/sync/", { fund_codes: fundCodes, start_date: startDate, end_date: endDate });

// watchlists
export const listWatchlists = () => api.get("/watchlists/");

export const createWatchlist = (name: string) => api.post("/watchlists/", { name });

export const addWatchlistItem = (watchlistId: string, fundCode: string) =>
  api.post(`/watchlists/${encodeURIComponent(watchlistId)}/items/`, { fund_code: fundCode });

