import api from './axios';

// 系统管理
export const healthCheck = () => api.get('/health/');

// Bootstrap 初始化
export const verifyBootstrapKey = (key) =>
  api.post('/admin/bootstrap/verify', { bootstrap_key: key });

export const initializeSystem = (data) =>
  api.post('/admin/bootstrap/initialize', data);

// 认证
export const login = (username, password) =>
  api.post('/auth/login', { username, password });

export const refreshToken = (refreshToken) =>
  api.post('/auth/refresh', { refresh_token: refreshToken });

export const getCurrentUser = () => api.get('/auth/me');

export const changePassword = (oldPassword, newPassword) =>
  api.put('/auth/password', {
    old_password: oldPassword,
    new_password: newPassword,
  });
