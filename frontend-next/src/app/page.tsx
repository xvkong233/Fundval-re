"use client";

import { Spin, Typography } from "antd";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { isAuthenticated } from "../lib/auth";

const { Text } = Typography;

export default function HomePage() {
  const router = useRouter();
  const [tip, setTip] = useState("正在检查系统状态...");

  useEffect(() => {
    let cancelled = false;

    async function run() {
      try {
        const res = await fetch("/api/health/", { headers: { Accept: "application/json" } });
        const data = (await res.json()) as any;

        if (cancelled) return;

        const initialized = data?.system_initialized;
        if (initialized === false) {
          router.replace("/initialize");
          return;
        }

        if (initialized === true && isAuthenticated()) {
          router.replace("/dashboard");
          return;
        }

        router.replace("/login");
      } catch {
        if (cancelled) return;
        setTip("无法连接到服务器，正在跳转登录页...");
        router.replace("/login");
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, [router]);

  return (
    <div
      style={{
        minHeight: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "#f0f2f5",
        flexDirection: "column",
        gap: 12,
      }}
    >
      <Spin size="large" />
      <Text type="secondary">{tip}</Text>
    </div>
  );
}

