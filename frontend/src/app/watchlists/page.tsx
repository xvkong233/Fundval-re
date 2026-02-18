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
  import {
    AutoComplete,
    Button,
    Card,
    Empty,
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
  import { useEffect, useMemo, useRef, useState } from "react";
  import { AuthedLayout } from "../../components/AuthedLayout";
  import {
    addWatchlistItem,
    batchEstimate,
    batchUpdateNav,
    createWatchlist,
    deleteWatchlist,
    listFunds,
    listSources,
    listWatchlists,
    patchWatchlist,
    removeWatchlistItem,
    reorderWatchlist,
  } from "../../lib/api";
  import { mergeBatchEstimate, mergeBatchNav, normalizeFundList, type Fund } from "../../lib/funds";
  import type { SourceItem } from "../../lib/sources";
  import { getFundCodes, moveInArray, pickDefaultWatchlistId, type Watchlist } from "../../lib/watchlists";

 const { Title, Text } = Typography;

 const FUND_SEARCH_PAGE_SIZE = 10;

  export default function WatchlistsPage() {
    const [loading, setLoading] = useState(false);
    const [refreshing, setRefreshing] = useState(false);
    const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
   const [activeWatchlistId, setActiveWatchlistId] = useState<string | null>(null);
   const [rows, setRows] = useState<Fund[]>([]);
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

    const loadSources = async () => {
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
    };

    const loadWatchlists = async (opts?: { preferActiveId?: string | null }) => {
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
   };

    const refreshEstimatesAndNavs = async (codes?: string[], opts?: { silent?: boolean }) => {
      const fundCodes = codes ?? getFundCodes(rows);
      if (fundCodes.length === 0) return;
      setRefreshing(true);
      try {
        const [estRes, navRes] = await Promise.all([
          batchEstimate(fundCodes, source),
          batchUpdateNav(fundCodes, source),
        ]);
        setRows((prev) => mergeBatchEstimate(mergeBatchNav(prev, navRes.data), estRes.data));
        setLastUpdateTime(new Date());
        if (!opts?.silent) message.success("数据已刷新");
      } catch {
       message.error("获取估值/净值失败");
     } finally {
       setRefreshing(false);
     }
   };

    useEffect(() => {
      void loadSources();
      void loadWatchlists();
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    useEffect(() => {
      if (typeof window === "undefined") return;
      const saved = window.localStorage.getItem("fundval_source");
      if (saved && saved.trim()) setSource(saved.trim());
    }, []);

    useEffect(() => {
      if (!sources.length) return;
      const has = sources.some((s) => String(s?.name ?? "") === source);
      if (!has) setSource(String(sources[0]?.name ?? "tiantian"));
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [sources]);

    useEffect(() => {
      if (typeof window === "undefined") return;
      window.localStorage.setItem("fundval_source", source);
    }, [source]);

   useEffect(() => {
     const base = Array.isArray(currentWatchlist?.items) ? (currentWatchlist?.items as Fund[]) : [];
     setRows(base.map((it) => ({ ...it, fund_code: String(it.fund_code) })));
     setDirtyOrder(false);
     setLastUpdateTime(null);
     const fundCodes = getFundCodes(base);
     if (fundCodes.length) void refreshEstimatesAndNavs(fundCodes, { silent: true });
     // eslint-disable-next-line react-hooks/exhaustive-deps
   }, [activeWatchlistId, currentWatchlist, source]);

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

   const handleRemoveFund = async (fundCode: string) => {
     if (!currentWatchlist) return;
     setLoading(true);
     try {
       await removeWatchlistItem(currentWatchlist.id, fundCode);
       message.success("已移除");
       await loadWatchlists({ preferActiveId: currentWatchlist.id });
     } catch (error: any) {
       const msg = error?.response?.data?.error || "移除失败";
       message.error(msg);
     } finally {
       setLoading(false);
     }
   };

   return (
     <AuthedLayout
       title={
         <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
           <span>自选</span>
           {lastUpdateTime ? (
             <Text type="secondary" style={{ fontSize: 12 }}>
               更新于 {lastUpdateTime.toLocaleTimeString()}
             </Text>
           ) : null}
         </div>
       }
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
             <Space style={{ width: "100%", justifyContent: "space-between" }} wrap>
               <Title level={4} style={{ margin: 0 }}>
                 {currentWatchlist.name ?? "自选列表"}
               </Title>
               <Space wrap>
                 <Button
                   icon={<EditOutlined />}
                   onClick={() => {
                     setRenameName(currentWatchlist.name ?? "");
                     setRenameOpen(true);
                   }}
                 >
                   重命名
                 </Button>
                 <Popconfirm
                   title="确认删除该自选列表？"
                   okText="删除"
                   cancelText="取消"
                   onConfirm={() => void handleDelete()}
                 >
                   <Button danger icon={<DeleteOutlined />}>
                     删除
                   </Button>
                 </Popconfirm>
               </Space>
             </Space>

             <Space style={{ width: "100%", justifyContent: "space-between" }} wrap>
               <Space wrap>
                 <AutoComplete
                   style={{ width: 360, maxWidth: "100%" }}
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
               </Space>

                <Space wrap>
                  <Select
                    style={{ minWidth: 160 }}
                    loading={sourcesLoading}
                    value={source}
                    onChange={(v) => setSource(String(v))}
                    options={(sources.length ? sources : [{ name: "tiantian" }]).map((s) => ({
                      label: s.name,
                      value: s.name,
                    }))}
                  />
                  <Tag color="blue">{source}</Tag>
                  <Button icon={<ReloadOutlined />} loading={refreshing} onClick={() => void refreshEstimatesAndNavs()}>
                    刷新估值/净值
                  </Button>
                  <Button
                   icon={<SaveOutlined />}
                   disabled={!dirtyOrder}
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
               </Space>
             </Space>

             {rows.length === 0 ? (
               <Empty description="还没有添加基金" />
             ) : (
               <Table<Fund>
                 rowKey={(r) => r.fund_code}
                 dataSource={rows}
                 pagination={false}
                 size="middle"
                 columns={[
                   { title: "代码", dataIndex: "fund_code", width: 110 },
                   { title: "基金名称", dataIndex: "fund_name", ellipsis: true },
                   {
                     title: "最新净值",
                     dataIndex: "latest_nav",
                     width: 150,
                     render: (nav: any, record) => {
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
                     width: 140,
                     render: (nav: any) => {
                       if (!nav) return "-";
                       const v = Number(nav);
                       return Number.isFinite(v) ? v.toFixed(4) : String(nav);
                     },
                   },
                   {
                     title: "估算涨跌(%)",
                     dataIndex: "estimate_growth",
                     width: 140,
                     render: (g: any) => {
                       if (g === undefined || g === null || g === "") return "-";
                       const v = Number(g);
                       const text = Number.isFinite(v) ? v.toFixed(2) : String(g);
                       const positive = Number.isFinite(v) ? v >= 0 : String(g).startsWith("-");
                       return (
                         <span style={{ color: positive ? "#cf1322" : "#3f8600" }}>
                           {Number.isFinite(v) && v >= 0 ? "+" : ""}
                           {text}
                         </span>
                       );
                     },
                   },
                   {
                     title: "操作",
                     key: "action",
                     width: 180,
                     render: (_, record, index) => (
                       <Space>
                         <Button
                           size="small"
                           icon={<ArrowUpOutlined />}
                           disabled={index === 0}
                           onClick={() => {
                             setRows((prev) => moveInArray(prev, index, index - 1));
                             setDirtyOrder(true);
                           }}
                         />
                         <Button
                           size="small"
                           icon={<ArrowDownOutlined />}
                           disabled={index === rows.length - 1}
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
                       </Space>
                     ),
                   },
                 ]}
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

