use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct RelateTheme {
    pub sec_code: String,
    pub sec_name: String,
    pub corr_1y: Option<f64>,
    pub ol2top: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct Jjxqy1_2Root {
    data: Option<Jjxqy1_2Data>,
}

#[derive(Debug, Deserialize)]
struct Jjxqy1_2Data {
    #[serde(default, rename = "fundRelateTheme")]
    fund_relate_theme: Vec<Jjxqy1_2ThemeItem>,
}

#[derive(Debug, Deserialize)]
struct Jjxqy1_2ThemeItem {
    #[serde(rename = "SEC_CODE")]
    sec_code: Option<String>,
    #[serde(rename = "SEC_NAME")]
    sec_name: Option<String>,
    #[serde(rename = "CORR_1Y")]
    corr_1y: Option<f64>,
    #[serde(rename = "OL2TOP")]
    ol2top: Option<f64>,
}

pub fn parse_fund_relate_themes_from_jjxqy1_2(payload: &str) -> Result<Vec<RelateTheme>, String> {
    let root: Jjxqy1_2Root =
        serde_json::from_str(payload).map_err(|e| format!("jjxqy1_2 JSON 解析失败: {e}"))?;

    let data = match root.data {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    let mut out = Vec::with_capacity(data.fund_relate_theme.len());
    for it in data.fund_relate_theme {
        let Some(sec_code) = it
            .sec_code
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(sec_name) = it
            .sec_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        out.push(RelateTheme {
            sec_code,
            sec_name,
            corr_1y: it.corr_1y,
            ol2top: it.ol2top,
        });
    }

    Ok(out)
}

fn stable_validmark_for_fund_code(fund_code: &str) -> String {
    // 生成稳定的 32 hex 字符串，避免每次请求都换 deviceid（更像真实客户端，也便于缓存）。
    let mut h1: u64 = 14695981039346656037;
    for b in fund_code.as_bytes() {
        h1 ^= *b as u64;
        h1 = h1.wrapping_mul(1099511628211);
    }
    let mut h2: u64 = 1099511628211;
    for b in b"fundval-tiantiantheme" {
        h2 ^= *b as u64;
        h2 = h2.wrapping_mul(14695981039346656037);
    }
    format!("{h1:016x}{h2:016x}")
}

pub async fn fetch_fund_relate_themes(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Vec<RelateTheme>, String> {
    let code = fund_code.trim();
    if code.is_empty() {
        return Ok(Vec::new());
    }

    // 复刻 h5.1234567.com.cn/app/fund-details 的 merge API 请求参数（否则会报“参数校验失败”）。
    const INDEXFIELDS: &str = "_id,INDEXCODE,BKID,INDEXNAME,INDEXVALUA,NEWINDEXTEXCH,PEP100";
    const FIELDS: &str = "BENCH,ESTDIFF,INDEXNAME,LINKZSB,INDEXCODE,NEWTEXCH,FTYPE,FCODE,BAGTYPE,RISKLEVEL,TTYPENAME,PTDT_FY,PTDT_TRY,PTDT_TWY,PTDT_Y,DWDT_FY,DWDT_TRY,DWDT_TWY,DWDT_Y,MBDT_FY,MBDT_TRY,MBDT_TWY,MBDT_Y,YDDT_FY,YDDT_TRY,YDDT_TWY,YDDT_Y,BFUNDTYPE,YMATCHCODEA,RLEVEL_SZ,RLEVEL_CX,ESTABDATE,JJGS,JJGSID,ENDNAV,FEGMRQ,SHORTNAME,TTYPE,TJDIN,FUNDEXCHG,LISTTEXCHMARK,FSRQ,ISSBDATE,ISSEDATE,FEATURE,DWJZ,LJJZ,MINRG,RZDF,PERIODNAME,SYL_1N,SYL_LN,SYL_Z,SOURCERATE,RATE,TSRQ,BTYPE,BUY,BENCHCODE,BENCH_CORR,TRKERROR,BENCHRATIO,NEWINDEXTEXCH,BESTDT_STRATEGY,BESTDT_Y,BESTDT_TWY,BESTDT_TRY,BESTDT_FY";
    const UNIQUE_FIELDS: &str = "FCODE,STDDEV1,STDDEV_1NRANK,STDDEV_1NFSC,STDDEV3,STDDEV_3NRANK,STDDEV_3NFSC,STDDEV5,STDDEV_5NRANK,STDDEV_5NFSC,SHARP1,SHARP_1NRANK,SHARP_1NFSC,SHARP3,SHARP_3NRANK,SHARP_3NFSC,SHARP5,SHARP_5NRANK,SHARP_5NFSC,MAXRETRA1,MAXRETRA_1NRANK,MAXRETRA_1NFSC,MAXRETRA3,MAXRETRA_3NRANK,MAXRETRA_3NFSC,MAXRETRA5,MAXRETRA_5NRANK,MAXRETRA_5NFSC,TRKERROR1,TRKERROR_1NRANK,TRKERROR_1NFSC,TRKERROR3,TRKERROR_3NRANK,TRKERROR_3NFSC,TRKERROR5,TRKERROR_5NRANK,TRKERROR_5NFSC";
    const UNIQUE_L_FIELDS: &str =
        "FCODE,BUSINESSTYPE,BUSINESSTEXT,BUSINESSCODE,BUSINESSSUBTYPE,MARK";
    const CFH_FIELDS: &str = "INVESTMENTIDEAR,INVESTMENTIDEARIMG";
    const RELATE_THEME_FIELDS: &str = "FCODE,SEC_CODE,SEC_NAME,CORR_1Y,OL2TOP";

    let validmark = stable_validmark_for_fund_code(code);

    let params = vec![
        ("deviceid", validmark.clone()),
        ("version", "9.9.9".to_string()),
        ("appVersion", "6.5.5".to_string()),
        ("product", "EFund".to_string()),
        ("plat", "Web".to_string()),
        ("uid", "".to_string()),
        ("fcode", code.to_string()),
        ("indexfields", INDEXFIELDS.to_string()),
        ("fields", FIELDS.to_string()),
        ("fundUniqueInfo_fIELDS", UNIQUE_FIELDS.to_string()),
        ("fundUniqueInfo_fLFIELDS", UNIQUE_L_FIELDS.to_string()),
        ("cfhFundFInfo_fields", CFH_FIELDS.to_string()),
        ("ISRG", "0".to_string()),
        ("relateThemeFields", RELATE_THEME_FIELDS.to_string()),
    ];

    let resp = client
        .post("https://dgs.tiantianfunds.com/merge/m/api/jjxqy1_2")
        .header("referer", "https://h5.1234567.com.cn/")
        .header("validmark", &validmark)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("jjxqy1_2 请求失败: {e}"))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("jjxqy1_2 读取响应失败: {e}"))?;

    parse_fund_relate_themes_from_jjxqy1_2(&text)
}

pub async fn upsert_fund_relate_themes(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source: &str,
    themes: &[RelateTheme],
) -> Result<i64, String> {
    let code = fund_code.trim();
    let source = source.trim();
    if code.is_empty() || source.is_empty() {
        return Ok(0);
    }

    let mut upserted = 0_i64;
    for t in themes {
        let sec_code = t.sec_code.trim();
        let sec_name = t.sec_name.trim();
        if sec_code.is_empty() || sec_name.is_empty() {
            continue;
        }

        let sql_pg = r#"
            INSERT INTO fund_relate_theme (
              fund_code, sec_code, sec_name, corr_1y, ol2top, source, fetched_at, created_at, updated_at
            ) VALUES (
              $1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            ON CONFLICT (fund_code, sec_code, source) DO UPDATE
              SET sec_name = EXCLUDED.sec_name,
                  corr_1y = EXCLUDED.corr_1y,
                  ol2top = EXCLUDED.ol2top,
                  fetched_at = CURRENT_TIMESTAMP,
                  updated_at = CURRENT_TIMESTAMP
        "#;

        let sql_any = r#"
            INSERT INTO fund_relate_theme (
              fund_code, sec_code, sec_name, corr_1y, ol2top, source, fetched_at, created_at, updated_at
            ) VALUES (
              $1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            ON CONFLICT (fund_code, sec_code, source) DO UPDATE
              SET sec_name = excluded.sec_name,
                  corr_1y = excluded.corr_1y,
                  ol2top = excluded.ol2top,
                  fetched_at = CURRENT_TIMESTAMP,
                  updated_at = CURRENT_TIMESTAMP
        "#;

        let r = sqlx::query(sql_pg)
            .bind(code)
            .bind(sec_code)
            .bind(sec_name)
            .bind(t.corr_1y)
            .bind(t.ol2top)
            .bind(source)
            .execute(pool)
            .await;

        if r.is_ok() {
            upserted += 1;
            continue;
        }

        sqlx::query(sql_any)
            .bind(code)
            .bind(sec_code)
            .bind(sec_name)
            .bind(t.corr_1y)
            .bind(t.ol2top)
            .bind(source)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        upserted += 1;
    }

    Ok(upserted)
}
