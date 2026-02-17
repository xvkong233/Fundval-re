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

export const changePassword = (oldPassword: string, newPassword: string) =>
  api.put("/auth/password", { old_password: oldPassword, new_password: newPassword });

export const getMySummary = () => api.get("/users/me/summary/");

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

export const patchWatchlist = (watchlistId: string, name: string) =>
  api.patch(`/watchlists/${encodeURIComponent(watchlistId)}/`, { name });

export const deleteWatchlist = (watchlistId: string) =>
  api.delete(`/watchlists/${encodeURIComponent(watchlistId)}/`);

export const removeWatchlistItem = (watchlistId: string, fundCode: string) =>
  api.delete(
    `/watchlists/${encodeURIComponent(watchlistId)}/items/${encodeURIComponent(fundCode)}/`
  );

export const reorderWatchlist = (watchlistId: string, fundCodes: string[]) =>
  api.put(`/watchlists/${encodeURIComponent(watchlistId)}/reorder/`, { fund_codes: fundCodes });

// accounts
export const listAccounts = () => api.get("/accounts/");

export const createAccount = (data: { name: string; parent?: string | null; is_default?: boolean }) =>
  api.post("/accounts/", data);

export const patchAccount = (
  accountId: string,
  data: { name?: string; parent?: string | null; is_default?: boolean }
) => api.patch(`/accounts/${encodeURIComponent(accountId)}/`, data);

export const deleteAccount = (accountId: string) =>
  api.delete(`/accounts/${encodeURIComponent(accountId)}/`);

// positions
export const listPositions = (params?: { account?: string }) => api.get("/positions/", { params });

export const listPositionOperations = (params?: { account?: string; fund_code?: string }) =>
  api.get("/positions/operations/", { params });

export const createPositionOperation = (data: {
  account: string;
  fund_code: string;
  operation_type: "BUY" | "SELL";
  operation_date: string;
  before_15: boolean;
  amount: string | number;
  share: string | number;
  nav: string | number;
}) => api.post("/positions/operations/", data);

export const deletePositionOperation = (operationId: string) =>
  api.delete(`/positions/operations/${encodeURIComponent(operationId)}/`);

export const recalculatePositions = (accountId?: string) =>
  api.post("/positions/recalculate/", accountId ? { account_id: accountId } : {});

export const queryFundNav = (data: { fund_code: string; operation_date: string; before_15: boolean }) =>
  api.post("/funds/query_nav/", data);

