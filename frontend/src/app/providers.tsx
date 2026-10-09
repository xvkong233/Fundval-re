"use client";

import React from "react";
import { App as AntdApp, ConfigProvider } from "antd";
import { AuthProvider } from "../contexts/AuthContext";

/**
 * Fundval Pro 设计主题
 * - 暖白底色 + 靛蓝主色，圆角 14px
 * - 中式涨跌色：红涨 (#DC2626) / 绿跌 (#16A34A)
 */
export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ConfigProvider
      theme={{
        token: {
          colorPrimary: "#4F46E5",
          colorInfo: "#4F46E5",
          colorSuccess: "#16A34A",
          colorWarning: "#D97706",
          colorError: "#DC2626",
          colorLink: "#4F46E5",
          borderRadius: 14,
          borderRadiusLG: 16,
          borderRadiusSM: 10,
          fontFamily: "var(--font-sans)",
          fontFamilyCode: "var(--font-mono)",
          colorBgLayout: "#F4F5F7",
          colorBgContainer: "#FFFFFF",
          colorBorderSecondary: "#ECEEF1",
          colorText: "#1A1D26",
          colorTextSecondary: "#6B7280",
          colorTextTertiary: "#9CA3AF",
          boxShadow: "0 1px 2px rgba(26, 29, 38, 0.04)",
          boxShadowSecondary: "0 8px 24px rgba(26, 29, 38, 0.08)",
        },
        components: {
          Layout: {
            headerBg: "#FFFFFF",
            bodyBg: "transparent",
            siderBg: "#FFFFFF",
          },
          Menu: {
            itemBg: "transparent",
            itemSelectedBg: "#EEF0FF",
            itemSelectedColor: "#4F46E5",
            itemHoverBg: "#F4F5F7",
            itemHoverColor: "#1A1D26",
            itemColor: "#4B5563",
            itemBorderRadius: 10,
            itemMarginInline: 8,
            itemHeight: 40,
            iconSize: 17,
          },
          Card: {
            headerFontSize: 15,
            headerFontSizeSM: 14,
            headerBg: "transparent",
            paddingLG: 20,
            borderRadiusLG: 16,
          },
          Table: {
            cellPaddingBlock: 12,
            cellPaddingInline: 16,
            headerBg: "#F8F9FB",
            headerColor: "#6B7280",
            headerSplitColor: "transparent",
            borderColor: "#F0F1F4",
            rowHoverBg: "#F8F9FE",
          },
          Tabs: {
            inkBarColor: "#4F46E5",
            itemSelectedColor: "#4F46E5",
            itemHoverColor: "#4F46E5",
          },
          Statistic: {
            titleFontSize: 13,
            contentFontSize: 26,
          },
          Button: {
            borderRadius: 10,
            controlHeight: 36,
            fontWeight: 500,
          },
          Input: {
            borderRadius: 10,
            controlHeight: 38,
          },
          Select: {
            borderRadius: 10,
            controlHeight: 38,
          },
          Tag: {
            borderRadiusSM: 8,
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
