"use client";

import React from "react";
import { App as AntdApp, ConfigProvider } from "antd";
import { AuthProvider } from "../contexts/AuthContext";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ConfigProvider
      theme={{
        token: {
          colorPrimary: "#1E40AF",
          colorInfo: "#1E40AF",
          colorSuccess: "#16A34A",
          colorWarning: "#F59E0B",
          colorError: "#DC2626",
          borderRadius: 10,
          fontFamily: "var(--font-sans)",
          fontFamilyCode: "var(--font-mono)",
        },
        components: {
          Layout: {
            headerBg: "#FFFFFF",
            bodyBg: "transparent",
            siderBg: "#0B1220",
          },
          Menu: {
            darkItemBg: "transparent",
            darkItemSelectedBg: "rgba(59, 130, 246, 0.16)",
            darkItemSelectedColor: "#E5E7EB",
            darkItemHoverBg: "rgba(255, 255, 255, 0.06)",
          },
          Card: {
            headerFontSize: 14,
            headerFontSizeSM: 13,
            headerBg: "transparent",
            paddingLG: 16,
          },
          Table: {
            cellPaddingBlock: 10,
            cellPaddingInline: 12,
            headerBg: "rgba(15, 23, 42, 0.03)",
          },
          Tabs: {
            cardPaddingSM: "8px 12px",
          },
          Statistic: {
            titleFontSize: 12,
            contentFontSize: 22,
          },
        },
      }}
    >
      <AntdApp>
        <AuthProvider>{children}</AuthProvider>
      </AntdApp>
    </ConfigProvider>
  );
}

