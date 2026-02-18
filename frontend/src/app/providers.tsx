"use client";

import React from "react";
import { App as AntdApp, ConfigProvider } from "antd";
import { AuthProvider } from "../contexts/AuthContext";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ConfigProvider>
      <AntdApp>
        <AuthProvider>{children}</AuthProvider>
      </AntdApp>
    </ConfigProvider>
  );
}

