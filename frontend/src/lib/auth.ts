"use client";

export function setToken(accessToken: string, refreshToken: string) {
  localStorage.setItem("access_token", accessToken);
  localStorage.setItem("refresh_token", refreshToken);
}

export function getToken(): { accessToken: string | null; refreshToken: string | null } {
  return {
    accessToken: localStorage.getItem("access_token"),
    refreshToken: localStorage.getItem("refresh_token"),
  };
}

export function clearToken() {
  localStorage.removeItem("access_token");
  localStorage.removeItem("refresh_token");
}

export function isAuthenticated() {
  return !!localStorage.getItem("access_token");
}

export function setUser(user: unknown) {
  localStorage.setItem("user", JSON.stringify(user));
}

export function getUser<T = any>(): T | null {
  const raw = localStorage.getItem("user");
  if (!raw) return null;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

export function logout() {
  clearToken();
  localStorage.removeItem("user");
}

