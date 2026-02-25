"use client";

  import {
    ArrowDownOutlined,
    ArrowUpOutlined,
    DeleteOutlined,
    EditOutlined,
    PlusOutlined,
    ReloadOutlined,
    SaveOutlined,
  } from "@ant-design/icons";
  import Link from "next/link";
    import {
      AutoComplete,
      Button,
      Card,
      Empty,
      Grid,
      Input,
      Modal,
      Popconfirm,
      Space,
      Select,
    Table,
    Tabs,
    Tag,
    Typography,
    message,
  } from "antd";
   import { useCallback, useEffect, useMemo, useRef, useState } from "react";
  import { AuthedLayout } from "../../components/AuthedLayout";
  import {
    addWatchlistItem,
    batchEstimate,
    createWatchlist,
    deleteWatchlist,
    getTaskJobDetail,
    listFunds,
    listSources,
    listWatchlists,
    patchWatchlist,
    refreshPricesBatchAsync,
    removeWatchlistItem,
    reorderWatchlist,
  } from "../../lib/api";
  import { mergeBatchEstimate, mergeBatchNav, normalizeFundList, type Fund } from "../../lib/funds";
  import { sourceDisplayName, type SourceItem } from "../../lib/sources";
  import { getFundCodes, moveInArray, pickDefaultWatchlistId, type Watchlist } from "../../lib/watchlists";

 const { Title, Text } = Typography;

 const FUND_SEARCH_PAGE_SIZE = 10;

   export default function WatchlistsPage() {
     const screens = Grid.useBreakpoint();
     const isMobile = !screens.md;
     const [loading, setLoading] = useState(false);
     const [refreshing, setRefreshing] = useState(false);
     const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
     const [activeWatchlistId, setActiveWatchlistId] = useState<string | null>(null);
     const [rows, setRows] = useState<Fund[]>([]);
    const [selectedFundCodes, setSelectedFundCodes] = useState<string[]>([]);
    const [filterKeyword, setFilterKeyword] = useState("");
    const [sortActive, setSortActive] = useState(false);
    const [dirtyOrder, setDirtyOrder] = useState(false);
    const [lastUpdateTime, setLastUpdateTime] = useState<Date | null>(null);

   const [createOpen, setCreateOpen] = useState(false);
   const [createName, setCreateName] = useState("");

   const [renameOpen, setRenameOpen] = useState(false);
   const [renameName, setRenameName] = useState("");

   const [searchKeyword, setSearchKeyword] = useState("");
   const [searchLoading, setSearchLoading] = useState(false);
    const [fundOptions, setFundOptions] = useState<Array<{ value: string; label: string }>>([]);
    const searchSeq = useRef(0);

    const [sourcesLoading, setSourcesLoading] = useState(false);
    const [sources, setSources] = useState<SourceItem[]>([]);
    const [source, setSource] = useState<string>("tiantian");

    const currentWatchlist = useMemo(() => {
      if (!activeWatchlistId) return null;
      return watchlists.find((w) => w.id === activeWatchlistId) ?? null;
    }, [activeWatchlistId, watchlists]);

     const loadSources = useCallback(async () => {
       setSourcesLoading(true);
       try {
         const res = await listSources();
         const list = Array.isArray(res.data) ? (res.data as SourceItem[]) : [];
         setSources(list);
       } catch {
         setSources([]);
       } finally {
         setSourcesLoading(false);
       }
     }, []);

     const loadWatchlists = useCallback(async (opts?: { preferActiveId?: string | null }) => {
       setLoading(true);
       try {
         const res = await listWatchlists();
        const list = Array.isArray(res.data) ? (res.data as Watchlist[]) : [];
        setWatchlists(list);
 
        const prefer = opts?.preferActiveId ?? activeWatchlistId;
        const stillExists = prefer ? list.some((w) => w.id === prefer) : false;
        const nextActive = stillExists ? prefer : pickDefaultWatchlistId(list);
        setActiveWatchlistId(nextActive);
      } catch (error: any) {
        const msg = error?.response?.data?.error || "加载自选列表失败";
        message.error(msg);
      } finally {
        setLoading(false);
      }
    }, [activeWatchlistId]);

     const refreshEstimatesAndNavs = async (codes?: string[], opts?: { silent?: boolean }) => {
       const fundCodes = codes ?? getFundCodes(rows);
       if (fundCodes.length === 0) return;
       setRefreshing(true);
       try {
         const r = await refreshPricesBatchAsync(fundCodes, source);
         const taskId = String(r?.data?.task_id ?? "").trim();
         if (taskId) {
           if (!opts?.silent) message.success(`已入队刷新任务：${taskId}`);
           const startedAt = Date.now();
           while (Date.now() - startedAt < 30 * 60 * 1000) {
             const d = await getTaskJobDetail(taskId);
             const status = String(d?.data?.job?.status ?? "").toLowerCase();
             if (status === "done") break;
             if (status === "error") {
                const err = String(d?.data?.job?.error ?? "任务执行失败");
                throw new Error(err);
             }
             await new Promise((resolve) => window.setTimeout(resolve, 1200));
           }
         }

         // 回读 DB 缓存（不再重复入队），刷新页面展示。
         const estRes = await batchEstimate(fundCodes, source, { enqueue_refresh: false });
         setRows((prev) => mergeBatchEstimate(mergeBatchNav(prev, estRes.data), estRes.data));
         setLastUpdateTime(new Date());
         if (!opts?.silent) message.success("数据已刷新");
       } catch (e: any) {
        const msg = e?.response?.data?.error || e?.message || "刷新失败";
        message.error(String(msg));
      } finally {
        setRefreshing(false);
      }
    };

     useEffect(() => {
       void loadSources();
       void loadWatchlists();
     }, [loadSources, loadWatchlists]);

    useEffect(() => {
      if (typeof window === "undefined") return;
      const saved = window.localStorage.getItem("fundval_source");
      if (saved && saved.trim()) setSource(saved.trim());
    }, []);

     useEffect(() => {
       if (!sources.length) return;
       const has = sources.some((s) => String(s?.name ?? "") === source);
       if (!has) setSource(String(sources[0]?.name ?? "tiantian"));
     }, [source, sources]);

    useEffect(() => {
      if (typeof window === "undefined") return;
      window.localStorage.setItem("fundval_source", source);
    }, [source]);

      useEffect(() => {
        const base = Array.isArray(currentWatchlist?.items) ? (currentWatchlist?.items as Fund[]) : [];
        setRows(base.map((it) => ({ ...it, fund_code: String(it.fund_code) })));
        setSelectedFundCodes([]);
        setFilterKeyword("");
        setSortActive(false);
        setDirtyOrder(false);
        setLastUpdateTime(null);
      }, [activeWatchlistId, currentWatchlist, source]);

    const viewRows = useMemo(() => {
      const q = filterKeyword.trim().toLowerCase();
      if (!q) return rows;
      return rows.filter((r) => {
        const code = String(r.fund_code ?? "").toLowerCase();
        const name = String(r.fund_name ?? "").toLowerCase();
        return code.includes(q) || name.includes(q);
      });
    }, [rows, filterKeyword]);

    const reorderDisabled = sortActive || Boolean(filterKeyword.trim());

    const handleCreate = async () => {
     if (!createName.trim()) {
       message.error("请输入自选列表名称");
       return;
     }
     setLoading(true);
     try {
       const resp = await createWatchlist(createName.trim());
       const id = resp.data?.id as string | undefined;
       message.success("创建成功");
       setCreateName("");
       setCreateOpen(false);
       await loadWatchlists({ preferActiveId: id ?? null });
     } catch (error: any) {
       const msg = error?.response?.data?.error || "创建失败";
       message.error(msg);
     } finally {
       setLoading(false);
     }
   };

   const handleRename = async () => {
     if (!currentWatchlist) return;
     if (!renameName.trim()) {
       message.error("请输入新名称");
       return;
     }
     setLoading(true);
     try {
       await patchWatchlist(currentWatchlist.id, renameName.trim());
       message.success("已更新");
       setRenameOpen(false);
       await loadWatchlists({ preferActiveId: currentWatchlist.id });
     } catch (error: any) {
       const msg = error?.response?.data?.error || "更新失败";
       message.error(msg);
     } finally {
       setLoading(false);
     }
   };

   const handleDelete = async () => {
     if (!currentWatchlist) return;
     setLoading(true);
     try {
       await deleteWatchlist(currentWatchlist.id);
       message.success("删除成功");
       await loadWatchlists({ preferActiveId: null });
     } catch (error: any) {
       const msg = error?.response?.data?.error || "删除失败";
       message.error(msg);
     } finally {
       setLoading(false);
     }
   };

   const handleSearchFunds = async (keyword: string) => {
     setSearchKeyword(keyword);
     const q = keyword.trim();
     if (!q) {
       setFundOptions([]);
       return;
     }

     const seq = ++searchSeq.current;
     setSearchLoading(true);
     try {
       const res = await listFunds({ page: 1, page_size: FUND_SEARCH_PAGE_SIZE, search: q });
       if (seq !== searchSeq.current) return;
       const normalized = normalizeFundList(res.data);
       setFundOptions(
         normalized.results
           .filter((f) => f.fund_code)
           .map((f) => ({
             value: f.fund_code,
             label: `${f.fund_code}${f.fund_name ? `  ${f.fund_name}` : ""}`,
           }))
       );
     } catch {
       if (seq !== searchSeq.current) return;
       setFundOptions([]);
     } finally {
       if (seq === searchSeq.current) setSearchLoading(false);
     }
   };

   const handleAddFund = async (fundCode: string) => {
     if (!currentWatchlist) return;
     const code = String(fundCode || "").trim();
     if (!code) return;

     setLoading(true);
     try {
       await addWatchlistItem(currentWatchlist.id, code);
       message.success("添加成功");
       setSearchKeyword("");
       setFundOptions([]);
       await loadWatchlists({ preferActiveId: currentWatchlist.id });
     } catch (error: any) {
       const msg = error?.response?.data?.error || "添加失败";
       message.error(msg);
     } finally {
       setLoading(false);
     }
   };

     const handleRemoveFund = useCallback(async (fundCode: string) => {
       const wid = currentWatchlist?.id;
       if (!wid) return;
       setLoading(true);
       try {
         await removeWatchlistItem(wid, fundCode);
         message.success("已移除");
         setSelectedFundCodes((prev) => prev.filter((x) => x !== fundCode));
         await loadWatchlists({ preferActiveId: wid });
       } catch (error: any) {
         const msg = error?.response?.data?.error || "移除失败";
         message.error(msg);
       } finally {
         setLoading(false);
       }
     }, [currentWatchlist?.id, loadWatchlists]);

    const handleBulkRemove = async () => {
      if (!currentWatchlist) return;
      const codes = selectedFundCodes.slice();
      if (!codes.length) return;
      setLoading(true);
      try {
        await Promise.all(codes.map((c) => removeWatchlistItem(currentWatchlist.id, c)));
        message.success(`已移除 ${codes.length} 个`);
        setSelectedFundCodes([]);
        await loadWatchlists({ preferActiveId: currentWatchlist.id });
      } catch (error: any) {
        const msg = error?.response?.data?.error || "批量移除失败";
        message.error(msg);
      } finally {
        setLoading(false);
      }
    };

    const columns = useMemo(() => {
      if (isMobile) {
        return [
          {
            title: "基金",
            key: "fund",
            render: (_: any, record: Fund) => {
              const code = String(record?.fund_code ?? "").trim();
              const name = String(record?.fund_name ?? "").trim();
              if (!code) return name || "-";
              return (
                <div style={{ minWidth: 0 }}>
                  <Link
                    href={`/funds/${encodeURIComponent(code)}`}
                    style={{
                      display: "block",
                      maxWidth: "100%",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                    title={name || code}
                  >
                    {name || code}
                  </Link>
                  <Text type="secondary" className="fv-mono" style={{ fontSize: 12, whiteSpace: "nowrap" }}>
                    {code}
                  </Text>
                </div>
              );
            },
          },
          {
            title: "净值/估值",
            key: "navs",
            render: (_: any, record: Fund) => {
              const latestNav = record?.latest_nav;
              const latestDate = typeof record?.latest_nav_date === "string" ? record.latest_nav_date : "";
              const latestDateStr = latestDate ? latestDate.slice(5) : "";

              const estimateNav = record?.estimate_nav;
              const estimateTime = typeof record?.estimate_time === "string" ? record.estimate_time : "";
              const estimateTimeStr = estimateTime && estimateTime.includes("T") ? estimateTime.slice(5, 16).replace("T", " ") : "";

              const estGrowth = record?.estimate_growth;
              const v = Number(estGrowth);
              const growthText =
                estGrowth === undefined || estGrowth === null || estGrowth === "" ? "-" : Number.isFinite(v) ? v.toFixed(2) : String(estGrowth);
              const positive = Number.isFinite(v) ? v >= 0 : !String(growthText).startsWith("-");

              const latestNavNum = Number(latestNav);
              const latestNavText = latestNav ? (Number.isFinite(latestNavNum) ? latestNavNum.toFixed(4) : String(latestNav)) : "-";
              const estimateNavNum = Number(estimateNav);
              const estimateNavText = estimateNav ? (Number.isFinite(estimateNavNum) ? estimateNavNum.toFixed(4) : String(estimateNav)) : "-";

              return (
                <div style={{ whiteSpace: "nowrap" }}>
                  <div>
                    <Text type="secondary" style={{ fontSize: 11, marginRight: 6 }}>
                      最新
                    </Text>
                    {latestNavText}
                    {latestDateStr ? (
                      <Text type="secondary" style={{ fontSize: 11, marginLeft: 6 }}>
                        {latestDateStr}
                      </Text>
                    ) : null}
                  </div>
                  <div>
                    <Text type="secondary" style={{ fontSize: 11, marginRight: 6 }}>
                      估值
                    </Text>
                    {estimateNavText}
                    {estimateTimeStr ? (
                      <Text type="secondary" style={{ fontSize: 11, marginLeft: 6 }}>
                        {estimateTimeStr}
                      </Text>
                    ) : null}
                  </div>
                  <div style={{ fontSize: 12, color: positive ? "#cf1322" : "#3f8600" }}>
                    {growthText !== "-" && Number.isFinite(v) && v >= 0 ? "+" : ""}
                    {growthText}
                    <Text type="secondary" style={{ marginLeft: 4, fontSize: 11 }}>
                      %
                    </Text>
                  </div>
                </div>
              );
            },
          },
          {
            title: "",
            key: "action",
            width: 154,
            render: (_: any, record: Fund, index: number) => {
              const code = String(record?.fund_code ?? "").trim();
              return (
                <div style={{ display: "flex", justifyContent: "flex-end", gap: 6, flexWrap: "nowrap" }}>
                  <Link href={`/funds/${encodeURIComponent(code)}`}>
                    <Button size="small">查看</Button>
                  </Link>
                  <Button
                    size="small"
                    icon={<ArrowUpOutlined />}
                    disabled={reorderDisabled || index === 0}
                    onClick={() => {
                      setRows((prev) => moveInArray(prev, index, index - 1));
                      setDirtyOrder(true);
                    }}
                  />
                  <Button
                    size="small"
                    icon={<ArrowDownOutlined />}
                    disabled={reorderDisabled || index === rows.length - 1}
                    onClick={() => {
                      setRows((prev) => moveInArray(prev, index, index + 1));
                      setDirtyOrder(true);
                    }}
                  />
                  <Popconfirm title={`移除 ${code}？`} okText="移除" cancelText="取消" onConfirm={() => void handleRemoveFund(code)}>
                    <Button size="small" danger icon={<DeleteOutlined />} />
                  </Popconfirm>
                </div>
              );
            },
          },
        ];
      }

      return [
        {
          title: "代码",
          dataIndex: "fund_code",
          width: 110,
          sorter: (a: any, b: any) => String(a?.fund_code ?? "").localeCompare(String(b?.fund_code ?? "")),
          render: (v: any, record: Fund) => {
            const code = String(record?.fund_code ?? v ?? "").trim();
            if (!code) return "-";
            return (
              <Link href={`/funds/${encodeURIComponent(code)}`} className="fv-mono" style={{ whiteSpace: "nowrap" }}>
                {code}
              </Link>
            );
          },
        },
        {
          title: "基金名称",
          dataIndex: "fund_name",
          ellipsis: true,
          sorter: (a: any, b: any) => String(a?.fund_name ?? "").localeCompare(String(b?.fund_name ?? "")),
          render: (v: any, record: Fund) => {
            const code = String(record?.fund_code ?? "").trim();
            const name = String(v ?? "").trim();
            if (!code) return name || "-";
            return (
              <Link
                href={`/funds/${encodeURIComponent(code)}`}
                style={{
                  display: "inline-block",
                  maxWidth: "100%",
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  verticalAlign: "bottom",
                }}
                title={name || code}
              >
                {name || code}
              </Link>
            );
          },
        },
        {
          title: "最新净值",
          dataIndex: "latest_nav",
          width: 150,
          sorter: (a: any, b: any) => Number(a?.latest_nav ?? -Infinity) - Number(b?.latest_nav ?? -Infinity),
          render: (nav: any, record: Fund) => {
            if (!nav) return "-";
            const date = record.latest_nav_date;
            const dateStr = typeof date === "string" ? `(${date.slice(5)})` : "";
            const v = Number(nav);
            return (
              <span style={{ whiteSpace: "nowrap" }}>
                {Number.isFinite(v) ? v.toFixed(4) : String(nav)}
                <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                  {dateStr}
                </Text>
              </span>
            );
          },
        },
        {
          title: "实时估值",
          dataIndex: "estimate_nav",
          width: 160,
          sorter: (a: any, b: any) => Number(a?.estimate_nav ?? -Infinity) - Number(b?.estimate_nav ?? -Infinity),
          render: (nav: any, record: Fund) => {
            if (!nav) return "-";
            const v = Number(nav);
            const text = Number.isFinite(v) ? v.toFixed(4) : String(nav);
            const t = typeof record?.estimate_time === "string" ? record.estimate_time : "";
            const tStr = t && t.includes("T") ? `(${t.slice(5, 16).replace("T", " ")})` : "";
            return (
              <span style={{ whiteSpace: "nowrap" }}>
                {text}
                {tStr ? (
                  <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                    {tStr}
                  </Text>
                ) : null}
              </span>
            );
          },
        },
        {
          title: "估算涨跌(%)",
          dataIndex: "estimate_growth",
          width: 140,
          sorter: (a: any, b: any) => Number(a?.estimate_growth ?? -Infinity) - Number(b?.estimate_growth ?? -Infinity),
          render: (g: any) => {
            if (g === undefined || g === null || g === "") return "-";
            const v = Number(g);
            const text = Number.isFinite(v) ? v.toFixed(2) : String(g);
            const positive = Number.isFinite(v) ? v >= 0 : !String(g).startsWith("-");
            return (
              <span style={{ color: positive ? "#cf1322" : "#3f8600", whiteSpace: "nowrap" }}>
                {Number.isFinite(v) && v >= 0 ? "+" : ""}
                {text}
              </span>
            );
          },
        },
        {
          title: "操作",
          key: "action",
          width: 200,
          render: (_: any, record: Fund, index: number) => (
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 6, flexWrap: "nowrap" }}>
              <Link href={`/funds/${encodeURIComponent(String(record?.fund_code ?? "").trim())}`}>
                <Button size="small">查看</Button>
              </Link>
              <Button
                size="small"
                icon={<ArrowUpOutlined />}
                disabled={reorderDisabled || index === 0}
                onClick={() => {
                  setRows((prev) => moveInArray(prev, index, index - 1));
                  setDirtyOrder(true);
                }}
              />
              <Button
                size="small"
                icon={<ArrowDownOutlined />}
                disabled={reorderDisabled || index === rows.length - 1}
                onClick={() => {
                  setRows((prev) => moveInArray(prev, index, index + 1));
                  setDirtyOrder(true);
                }}
              />
              <Popconfirm
                title={`移除 ${record.fund_code}？`}
                okText="移除"
                cancelText="取消"
                onConfirm={() => void handleRemoveFund(record.fund_code)}
              >
                <Button size="small" danger>
                  移除
                </Button>
              </Popconfirm>
            </div>
          ),
        },
      ];
    }, [handleRemoveFund, isMobile, reorderDisabled, rows.length]);

    return (
      <AuthedLayout
        title="自选"
        subtitle={lastUpdateTime ? `更新于 ${lastUpdateTime.toLocaleTimeString()}` : undefined}
      >
        <Card>
          <Tabs
           activeKey={activeWatchlistId ?? undefined}
           onChange={(k) => setActiveWatchlistId(k)}
           items={watchlists.map((w) => ({ key: w.id, label: w.name ?? w.id }))}
           tabBarExtraContent={
             <Button icon={<PlusOutlined />} onClick={() => setCreateOpen(true)}>
               新建
             </Button>
           }
         />

         {!currentWatchlist ? (
           <Empty description="还没有自选列表">
             <Button type="primary" icon={<PlusOutlined />} onClick={() => setCreateOpen(true)}>
               创建自选列表
             </Button>
           </Empty>
         ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              <div className="fv-toolbar">
                <div className="fv-toolbarLeft" style={{ minWidth: 0 }}>
                  <Title
                    level={4}
                    style={{ margin: 0, whiteSpace: "nowrap" }}
                    ellipsis={{ tooltip: currentWatchlist.name ?? "自选列表" }}
                  >
                    {currentWatchlist.name ?? "自选列表"}
                  </Title>
                </div>
                <div className="fv-toolbarRight fv-toolbarScroll">
                  <Button
                    icon={<EditOutlined />}
                    onClick={() => {
                      setRenameName(currentWatchlist.name ?? "");
                      setRenameOpen(true);
                    }}
                  >
                    重命名
                  </Button>
                  <Popconfirm title="确认删除该自选列表？" okText="删除" cancelText="取消" onConfirm={() => void handleDelete()}>
                    <Button danger icon={<DeleteOutlined />}>
                      删除
                    </Button>
                  </Popconfirm>
                </div>
              </div>

              <div className="fv-toolbar">
                <div className="fv-toolbarLeft fv-toolbarScroll">
                  <AutoComplete
                    style={{ width: isMobile ? 240 : 360, maxWidth: "100%" }}
                    value={searchKeyword}
                    options={fundOptions}
                    onSearch={(v) => void handleSearchFunds(v)}
                    onSelect={(v) => void handleAddFund(v)}
                    onChange={(v) => setSearchKeyword(v)}
                    placeholder="搜索基金代码或名称"
                    notFoundContent={searchLoading ? <Text type="secondary">搜索中…</Text> : null}
                  />
                  <Button
                    type="primary"
                    icon={<PlusOutlined />}
                    disabled={!searchKeyword.trim()}
                    onClick={() => void handleAddFund(searchKeyword)}
                  >
                    添加
                  </Button>
                </div>

                <div className="fv-toolbarRight fv-toolbarScroll">
                  <Select
                    style={{ minWidth: 160 }}
                    loading={sourcesLoading}
                    value={source}
                    onChange={(v) => setSource(String(v))}
                    options={(sources.length ? sources : [{ name: "tiantian" }]).map((s) => ({
                      label: `${sourceDisplayName(s.name)} (${s.name})`,
                      value: s.name,
                    }))}
                  />
                  {!isMobile ? <Tag color="blue">{sourceDisplayName(source)}</Tag> : null}
                  <Button icon={<ReloadOutlined />} loading={refreshing} onClick={() => void refreshEstimatesAndNavs()}>
                    刷新估值/净值
                  </Button>
                  <Button
                    icon={<SaveOutlined />}
                    disabled={!dirtyOrder || reorderDisabled}
                    onClick={async () => {
                      if (!currentWatchlist) return;
                      const codes = getFundCodes(rows);
                      if (!codes.length) return;
                      setLoading(true);
                      try {
                        await reorderWatchlist(currentWatchlist.id, codes);
                        message.success("排序已保存");
                        await loadWatchlists({ preferActiveId: currentWatchlist.id });
                      } catch (error: any) {
                        const msg = error?.response?.data?.error || "保存失败";
                        message.error(msg);
                      } finally {
                        setLoading(false);
                      }
                    }}
                  >
                    保存排序
                  </Button>
                </div>
              </div>

              <div className="fv-toolbar">
                <div className="fv-toolbarLeft fv-toolbarScroll">
                  <Input
                    allowClear
                    placeholder="筛选（代码/名称）"
                    style={{ width: isMobile ? 220 : 260, maxWidth: "100%" }}
                    value={filterKeyword}
                    onChange={(e) => setFilterKeyword(e.target.value)}
                  />
                  {reorderDisabled ? (
                    <Text type="secondary" style={{ fontSize: 12, whiteSpace: "nowrap" }}>
                      已启用筛选/排序：排序调整将暂时禁用
                    </Text>
                  ) : null}
                </div>

                <div className="fv-toolbarRight fv-toolbarScroll">
                  <Tag color={selectedFundCodes.length ? "blue" : "default"} style={{ whiteSpace: "nowrap" }}>
                    {`已选 ${selectedFundCodes.length}`}
                  </Tag>
                  <Button
                    disabled={selectedFundCodes.length === 0}
                    onClick={() => void refreshEstimatesAndNavs(selectedFundCodes)}
                    loading={refreshing}
                  >
                    刷新已选
                  </Button>
                  <Popconfirm
                    title={`批量移除 ${selectedFundCodes.length} 个基金？`}
                    okText="移除"
                    cancelText="取消"
                    okButtonProps={{ danger: true }}
                    onConfirm={() => void handleBulkRemove()}
                    disabled={selectedFundCodes.length === 0}
                  >
                    <Button danger disabled={selectedFundCodes.length === 0}>
                      批量移除
                    </Button>
                  </Popconfirm>
                  <Button disabled={selectedFundCodes.length === 0} onClick={() => setSelectedFundCodes([])}>
                    清空选择
                  </Button>
                </div>
              </div>

              {rows.length === 0 ? (
                <Empty description="还没有添加基金" />
              ) : (
                <Table<Fund>
                  rowKey={(r) => r.fund_code}
                  dataSource={viewRows}
                  pagination={{
                    pageSize: isMobile ? 10 : 20,
                    showSizeChanger: !isMobile,
                    showQuickJumper: !isMobile,
                    simple: isMobile,
                    showLessItems: isMobile,
                  }}
                  size={isMobile ? "small" : "middle"}
                  rowSelection={{
                    selectedRowKeys: selectedFundCodes,
                    onChange: (keys) => setSelectedFundCodes(keys.map((k) => String(k))),
                    preserveSelectedRowKeys: true,
                  }}
                  onChange={(_, __, sorter) => {
                    const active = Array.isArray(sorter)
                      ? sorter.some((s) => Boolean((s as any)?.order))
                      : Boolean((sorter as any)?.order);
                    setSortActive(active);
                  }}
                  columns={columns as any}
                />
              )}
            </div>
          )}
       </Card>

       <Modal
         title="新建自选列表"
         open={createOpen}
         onOk={() => void handleCreate()}
         confirmLoading={loading}
         onCancel={() => setCreateOpen(false)}
         okText="创建"
         cancelText="取消"
       >
         <Space direction="vertical" style={{ width: "100%" }}>
           <Text type="secondary">名称</Text>
           <Input
             value={createName}
             onChange={(e) => setCreateName(e.target.value)}
             placeholder="例如：我的自选"
             maxLength={32}
           />
         </Space>
       </Modal>

       <Modal
         title="重命名自选列表"
         open={renameOpen}
         onOk={() => void handleRename()}
         confirmLoading={loading}
         onCancel={() => setRenameOpen(false)}
         okText="保存"
         cancelText="取消"
       >
         <Space direction="vertical" style={{ width: "100%" }}>
           <Text type="secondary">新名称</Text>
           <Input
             value={renameName}
             onChange={(e) => setRenameName(e.target.value)}
             placeholder="例如：我的自选"
             maxLength={32}
           />
         </Space>
       </Modal>
     </AuthedLayout>
   );
 }

