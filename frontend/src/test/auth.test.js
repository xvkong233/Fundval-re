import { describe, it, expect } from 'vitest';
import { setToken, getToken, clearToken, isAuthenticated } from '../utils/auth';

describe('Auth Utils', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('setToken 保存 token 到 localStorage', () => {
    setToken('access123', 'refresh456');

    expect(localStorage.getItem('access_token')).toBe('access123');
    expect(localStorage.getItem('refresh_token')).toBe('refresh456');
  });

  it('getToken 从 localStorage 获取 token', () => {
    localStorage.setItem('access_token', 'access123');
    localStorage.setItem('refresh_token', 'refresh456');

    const tokens = getToken();

    expect(tokens.accessToken).toBe('access123');
    expect(tokens.refreshToken).toBe('refresh456');
  });

  it('clearToken 清除 localStorage 中的 token', () => {
    localStorage.setItem('access_token', 'access123');
    localStorage.setItem('refresh_token', 'refresh456');

    clearToken();

    expect(localStorage.getItem('access_token')).toBeNull();
    expect(localStorage.getItem('refresh_token')).toBeNull();
  });

  it('isAuthenticated 检查是否已认证', () => {
    expect(isAuthenticated()).toBe(false);

    localStorage.setItem('access_token', 'access123');
    expect(isAuthenticated()).toBe(true);

    clearToken();
    expect(isAuthenticated()).toBe(false);
  });
});
