export const setToken = (accessToken, refreshToken) => {
  localStorage.setItem('access_token', accessToken);
  localStorage.setItem('refresh_token', refreshToken);
};

export const getToken = () => {
  return {
    accessToken: localStorage.getItem('access_token'),
    refreshToken: localStorage.getItem('refresh_token'),
  };
};

export const clearToken = () => {
  localStorage.removeItem('access_token');
  localStorage.removeItem('refresh_token');
};

export const isAuthenticated = () => {
  return !!localStorage.getItem('access_token');
};
