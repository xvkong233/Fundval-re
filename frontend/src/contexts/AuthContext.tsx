"use client";

import React, { createContext, useContext, useEffect, useMemo, useState } from "react";
import { getUser, isAuthenticated, logout as clearAuthStorage, setUser as persistUser } from "../lib/auth";

type AuthContextValue = {
  user: any | null;
  loading: boolean;
  login: (userData: any) => void;
  logout: () => void;
  updateUser: (userData: any) => void;
  isAuthenticated: boolean;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<any | null>(null);
  const [loading, setLoading] = useState(true);
  const [authed, setAuthed] = useState(false);

  useEffect(() => {
    const storedUser = getUser();
    if (storedUser) setUser(storedUser);
    setAuthed(isAuthenticated());
    setLoading(false);
  }, []);

  const value = useMemo<AuthContextValue>(() => {
    return {
      user,
      loading,
      login: (userData) => {
        setUser(userData);
        persistUser(userData);
        setAuthed(true);
      },
      logout: () => {
        setUser(null);
        setAuthed(false);
        clearAuthStorage();
      },
      updateUser: (userData) => {
        setUser(userData);
        persistUser(userData);
      },
      isAuthenticated: authed,
    };
  }, [user, loading, authed]);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) throw new Error("useAuth must be used within an AuthProvider");
  return context;
}

