use axum::{Router, Json, response::Html};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use axum::middleware::{from_fn, Next};
use axum::body::Body;
use axum::extract::ConnectInfo;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use axum::extract::Json as AxumJson;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{Value, Map, json};
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesSettings {
    pub rate_limit_enabled: bool,
    pub rate_limit_per_second: u32,
    pub waf_enabled: bool,
    pub oauth2_enabled: bool,
    pub admin_console_enabled: bool,
    pub disabled_modules: Vec<String>,
    pub disabled_routes: Vec<String>,
    pub disabled_features: Vec<String>,
    pub oauth2_protected_routes: Vec<String>,
    pub route_rate_limits: HashMap<String, u32>,
    pub feature_extras: Map<String, Value>,
}

impl Default for FeaturesSettings {
    fn default() -> Self {
        Self {
            rate_limit_enabled: false,
            rate_limit_per_second: 1,
            waf_enabled: false,
            oauth2_enabled: false,
            admin_console_enabled: true,
            disabled_modules: Vec::new(),
            disabled_routes: Vec::new(),
            disabled_features: Vec::new(),
            oauth2_protected_routes: Vec::new(),
            route_rate_limits: HashMap::new(),
            feature_extras: Map::new(),
        }
    }
}

pub fn load_settings() -> FeaturesSettings {
    let path = std::path::Path::new("./admin/config/features.json");
    if let Ok(text) = std::fs::read_to_string(path) {
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        FeaturesSettings::default()
    }
}

pub fn save_settings(s: &FeaturesSettings) -> std::io::Result<()> {
    let path = std::path::Path::new("./admin/config/features.json");
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).ok(); }
    let text = serde_json::to_string_pretty(s).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(path, text)
}

// Middleware chặn truy cập /admin và /admin/settings chỉ cho phép từ 127.0.0.1 và Host=localhost
async fn admin_access_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    // Chỉ áp dụng guard cho đúng hai endpoint yêu cầu
    let must_guard = path == "/admin" || path == "/admin/" || path == "/admin/settings";
    if !must_guard {
        return Ok(next.run(req).await);
    }

    // Kiểm tra Host header
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host_base = host.split(':').next().unwrap_or("");
    let host_ok = host_base.eq_ignore_ascii_case("localhost");

    // Lấy IP client từ ConnectInfo (cần into_make_service_with_connect_info ở app)
    let ip_ok = if let Some(ci) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        match ci.0.ip() {
            IpAddr::V4(v4) => v4 == Ipv4Addr::LOCALHOST,
            IpAddr::V6(v6) => v6.is_loopback(),
        }
    } else {
        false
    };

    if host_ok && ip_ok {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

// Build admin router to be nested under "/admin"
pub fn build_router(
    live_spec: Arc<RwLock<Value>>,
    reload_fn: Arc<dyn Fn() + Send + Sync + 'static>,
) -> Router {
    Router::new()
        .route("/", axum::routing::get({
            let live_spec = live_spec.clone();
            move || async move {
                let spec = live_spec.read().clone();
                let mut routes: Vec<String> = Vec::new();
                if let Some(paths) = spec.get("paths").and_then(|v| v.as_object()) {
                    routes.extend(paths.keys().cloned());
                    routes.sort();
                }
                let mut modules: Vec<String> = Vec::new();
                if let Ok(rd) = std::fs::read_dir("./modules") {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.join("Cargo.toml").exists() {
                            if let Some(name) = p.file_name().map(|s| s.to_string_lossy().to_string()) {
                                modules.push(name);
                            }
                        }
                    }
                    modules.sort();
                }
    let body = r##"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <title>Admin Console</title>
  <style>
    :root { --bg:#0b1220; --panel:#131a2b; --muted:#cbd5e1; --accent:#4f46e5; --ok:#16a34a; --warn:#f59e0b; --text:#e2e8f0; }
    * { box-sizing: border-box; }
    body { font-family: Inter, system-ui, Arial, sans-serif; margin: 0; background: var(--bg); color: var(--text); }
    .topbar { position: sticky; top:0; z-index: 10; display:flex; align-items:center; justify-content:space-between; padding: 12px 20px; border-bottom: 1px solid #23304d; background: #0d1424cc; backdrop-filter: blur(6px); }
    .brand { font-weight: 600; letter-spacing: 0.2px; }
    .actions { display:flex; gap:8px; }
    .searchbar { flex:1; display:flex; align-items:center; gap:8px; margin: 0 16px; }
    .searchbar input { width:100%; padding:8px 12px; border-radius: 10px; border: 1px solid #23304d; background: #0f172a; color: var(--text); }
    .layout { display:grid; grid-template-columns: 240px 1fr; gap: 16px; padding: 20px; }
    .sidebar { background: var(--panel); border: 1px solid #23304d; border-radius: 12px; padding: 14px; position: sticky; top: 60px; height: calc(100vh - 80px); overflow:auto; }
    .menu { display:flex; flex-direction:column; gap:8px; margin-bottom: 12px; }
    .menu a { text-decoration:none; color: var(--text); padding:8px 10px; border-radius:8px; border:1px solid transparent; }
    .menu a:hover { border-color: var(--accent); background:#0f172a; }
    .menu a.active { border-color: var(--accent); background:#0f172a; }
    .menu a.nav-sub { padding-left: 22px; color: #a8b3c9; }
    .menu a.nav-sub:hover { color: var(--text); }
    .stats { display:grid; grid-template-columns: 1fr; gap:8px; border-top: 1px solid #23304d; padding-top: 12px; }
    .stats div { display:flex; align-items:center; justify-content:space-between; padding:6px 8px; background:#0f172a; border-radius:8px; border:1px solid #23304d; }
    .content { display:grid; grid-template-columns: 1fr; gap: 16px; min-width: 0; }
    .section h2 { margin: 0 0 8px 0; }
    .card { background: var(--panel); border: 1px solid #23304d; border-radius: 12px; padding: 16px; }
    .card.nested { margin-left: 16px; border-color: #2a3b5f; }
    /* Command Palette */
    .cmd-overlay { position: fixed; inset:0; background: #00000088; backdrop-filter: blur(2px); display:none; align-items: center; justify-content: center; z-index: 100; }
    .cmd-box { width: 640px; max-width: 94vw; background: #0f172a; border: 1px solid #23304d; border-radius: 14px; padding: 14px; box-shadow: 0 8px 24px #00000055; }
    .cmd-box h3 { margin:0 0 8px 0; }
    .cmd-input { width: 100%; padding: 10px 12px; border-radius: 10px; border: 1px solid #23304d; background: #0b1220; color: var(--text); }
    .cmd-hints { display:flex; flex-wrap:wrap; gap:8px; margin-top:8px; }
    .hint { padding:4px 8px; border:1px solid #23304d; border-radius: 8px; font-size:12px; color:#a8b3c9; cursor:pointer; }
    .hint:hover { color: var(--text); border-color: var(--accent); }
    .toggle { display: inline-block; margin-left: 8px; }
    code { background: #0f172a; padding: 2px 6px; border-radius: 6px; border: 1px solid #23304d; }
    ul { list-style: none; padding: 0; }
    li { margin: 6px 0; }
    button { padding: 8px 12px; border-radius: 8px; border: 1px solid #23304d; background: #0f172a; color: var(--text); cursor: pointer; }
    button:hover { border-color: var(--accent); }
    .subtitle { color: var(--muted); font-size: 12px; margin-top: 4px; }
    select { padding: 6px 8px; border-radius: 8px; border: 1px solid #23304d; background: #0f172a; color: var(--text); }
    /* Tabs */
    .tabs { display: flex; gap: 8px; border-bottom: 1px solid #23304d; margin-bottom: 12px; }
    .tab-btn { padding: 8px 12px; border-radius: 8px 8px 0 0; border: 1px solid #23304d; border-bottom: none; background: #0f172a; color: #e2e8f0; cursor: pointer; }
    .tab-btn.active { background: var(--panel); border-color: var(--accent); }
    .tab-content { padding: 8px; }
    @media (max-width: 900px) {
      .layout { grid-template-columns: 1fr; }
      .sidebar { position: relative; height: auto; top:0; }
    }
  </style>
  <script>
    async function fetchSettings() {
      const res = await fetch('/admin/settings');
      if (!res.ok) { throw new Error('Failed to load /admin/settings: '+res.status); }
      return res.json();
    }
    async function fetchManifests() {
      const res = await fetch('/admin/features-manifest');
      if (!res.ok) { return []; }
      return res.json();
    }
    async function fetchRoutes() {
      const res = await fetch('/admin/routes');
      if (!res.ok) { throw new Error('Failed to load /admin/routes: '+res.status); }
      return res.json();
    }
    let disabledRoutes = [];
    let disabledFeatures = [];
    let featureExtras = {};
    let featureManifests = [];
    let enabledFeatures = { oauth2:false, rate_limit:false, waf:false };
    // Global search
    function setupGlobalSearch() {
      const input = document.getElementById('global-search');
      if (!input) return;
      input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
          const q = (input.value||'').trim().toLowerCase();
          if (q.startsWith('routes')) { showSection('routes'); return; }
          if (q.startsWith('modules')) { showSection('modules'); return; }
          if (q.startsWith('features')) { showSection('features'); return; }
          // quick filter routes by module name
          const sel = document.getElementById('routes-filter');
          if (sel && q.length>0) {
            for (const opt of Array.from(sel.options)) { if (opt.value.toLowerCase().includes(q)) { sel.value = opt.value; break; } }
          }
          const rg = window.__routes_groups || {groups:[]}; renderRoutes(rg.groups);
        }
      });
    }
    function renderRoutes(groups) {
      const ul = document.getElementById('routes-list');
      ul.innerHTML = '';
      const sel = document.getElementById('routes-filter');
      const filterModule = sel ? sel.value : '';
      const toShow = (filterModule && filterModule.length>0) ? (groups||[]).filter(g=>g.module===filterModule) : (groups||[]);
      let count = 0;
      for (const g of toShow) {
        const li = document.createElement('li');
        li.innerHTML = '<strong><code>'+g.module+'</code></strong>';
        const inner = document.createElement('ul');
        inner.style.listStyle = 'disc';
        inner.style.marginLeft = '18px';
        for (const r of g.routes) {
          const rli = document.createElement('li');
          const chk = document.createElement('input');
          chk.type = 'checkbox'; chk.className='toggle';
          chk.checked = !(disabledRoutes||[]).includes(r);
          chk.onchange = () => {
            const cur = new Set(disabledRoutes||[]);
            if (!chk.checked) cur.add(r); else cur.delete(r);
            disabledRoutes = Array.from(cur);
          };
          rli.innerHTML = '<code>'+r+'</code>:';
          rli.appendChild(chk);
          inner.appendChild(rli);
          count++;
        }
        li.appendChild(inner);
        ul.appendChild(li);
      }
      const cntEl = document.getElementById('routes-count');
      if (cntEl) { cntEl.textContent = '('+count+' routes)'; }
    }
    // Command palette: simple NLP for VN/EN actions
    function openCmdPalette(open) {
      const overlay = document.getElementById('cmd-overlay');
      const input = document.getElementById('cmd-input');
      if (!overlay || !input) return;
      overlay.style.display = open ? 'flex' : 'none';
      if (open) { input.value=''; setTimeout(()=>input.focus(), 50); }
    }
    function parseCommand(text) {
      const t = (text||'').trim().toLowerCase();
      const cmd = { action:null, target:null, extra:null };
      // enable/disable feature
      if (/(bật|enable)\s+(oauth2|rate limit|waf)/.test(t)) { cmd.action='enable-feature'; cmd.target=t.match(/(oauth2|rate limit|waf)/)[0].replace(/\s+/g,'_'); return cmd; }
      if (/(tắt|disable)\s+(oauth2|rate limit|waf)/.test(t)) { cmd.action='disable-feature'; cmd.target=t.match(/(oauth2|rate limit|waf)/)[0].replace(/\s+/g,'_'); return cmd; }
      // open config
      if (/(mở|open)\s+(cấu hình|config)\s+(oauth2|rate limit|waf)/.test(t)) { cmd.action='open-config'; cmd.target=t.match(/(oauth2|rate limit|waf)/)[0].replace(/\s+/g,'_'); return cmd; }
      // navigate
      if (/(đi tới|goto|open)\s+(routes|modules|features|overview)/.test(t)) { cmd.action='navigate'; cmd.target=t.match(/(routes|modules|features|overview)/)[0]; return cmd; }
      // filter routes by module
      const m = t.match(/(lọc|filter)\s+routes\s+(module\s+)?([a-z0-9_]+)/);
      if (m) { cmd.action='filter-routes'; cmd.target=m[3]; return cmd; }
      return cmd;
    }
    async function executeCommand(cmd) {
      if (!cmd || !cmd.action) return;
      if (cmd.action==='navigate') { showSection(cmd.target==='overview'?'overview':cmd.target); return; }
      if (cmd.action==='filter-routes') {
        const sel = document.getElementById('routes-filter'); if (sel) { sel.value = cmd.target; }
        const rg = window.__routes_groups || {groups:[]}; renderRoutes(rg.groups); showSection('routes'); return; }
      if (cmd.action==='enable-feature' || cmd.action==='disable-feature') {
        const feat = cmd.target; const on = cmd.action==='enable-feature';
        const key = feat+'_enabled'; const map = { rate_limit_enabled:'rate_limit_enabled', waf_enabled:'waf_enabled', oauth2_enabled:'oauth2_enabled' };
        const settingsKey = map[key] || key;
        await updateSettings({ [settingsKey]: on });
        enabledFeatures[feat] = on;
        await fetch('/admin/reload',{ method:'POST' });
        const routes2 = await fetchRoutes(); window.__routes_groups = routes2;
        renderFeatureTabs(featureManifests, routes2.groups);
        if (on) { const tabsEl = document.getElementById('feature-tabs'); if (tabsEl) tabsEl.dataset.active = feat; showSection('feature-config'); }
        else { showSection('features'); }
        return;
      }
      if (cmd.action==='open-config') {
        const feat = cmd.target; const tabsEl = document.getElementById('feature-tabs'); if (tabsEl) tabsEl.dataset.active = feat;
        showSection('feature-config'); return;
      }
    }
    function setupCommandPalette() {
      const overlay = document.getElementById('cmd-overlay');
      const input = document.getElementById('cmd-input');
      const hints = document.querySelectorAll('.hint');
      if (!overlay || !input) return;
      document.addEventListener('keydown', (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase()==='k') { e.preventDefault(); openCmdPalette(true); }
        if (e.key==='Escape') { openCmdPalette(false); }
      });
      input.addEventListener('keydown', async (e) => {
        if (e.key==='Enter') { const cmd = parseCommand(input.value); await executeCommand(cmd); openCmdPalette(false); }
      });
      hints.forEach(h => h.addEventListener('click', async () => { const cmd = parseCommand(h.dataset.cmd||''); await executeCommand(cmd); openCmdPalette(false); }));
    }
    function generateSmartSuggestions() {
      const box = document.getElementById('smart-hints'); if (!box) return;
      box.innerHTML = '';
      const totalRoutes = ((window.__routes_groups||{groups:[]}).groups||[]).reduce((acc,g)=>acc + (g.routes||[]).length, 0);
      const push = (t,cmd) => { const el = document.createElement('span'); el.className='hint'; el.textContent=t; el.dataset.cmd=cmd; box.appendChild(el); };
      if (!enabledFeatures.oauth2) push('Bật OAuth2', 'bật oauth2');
      if (!enabledFeatures.rate_limit && totalRoutes>10) push('Bật Rate Limit', 'bật rate limit');
      if (!enabledFeatures.waf) push('Bật WAF', 'bật waf');
      push('Đi tới Modules', 'open modules');
      push('Mở cấu hình Rate Limit', 'mở cấu hình rate limit');
    }
    function renderSingleFeatureSettings(m, groups) {
      const container = document.getElementById('feature-tab-content');
      if (!container) return;
      container.innerHTML = '';
      const allRoutes = [];
      for (const g of groups) { for (const r of g.routes) allRoutes.push(r); }
      allRoutes.sort();
      const ex = featureExtras[m.name] || {};
      const ul = document.createElement('ul');
      ul.style.listStyle = 'none'; ul.style.padding = '0';
      for (const f of (m.settings||[])) {
        const li = document.createElement('li');
        const label = document.createElement('div'); label.textContent = f.label || f.key; li.appendChild(label);
        if (f.type === 'number') {
          const input = document.createElement('input');
          input.type = 'number'; input.value = (ex[f.key] ?? f.default ?? 0);
          input.onchange = () => { featureExtras[m.name] = featureExtras[m.name]||{}; featureExtras[m.name][f.key] = Number(input.value); scheduleSaveFeatureExtras(); };
          li.appendChild(input);
        } else if (f.type === 'string_list') {
          const ta = document.createElement('textarea');
          ta.placeholder = 'One per line'; ta.rows = 4; ta.cols = 24;
          const cur = Array.isArray(ex[f.key]) ? ex[f.key] : (f.default||[]);
          ta.value = cur.join('\n');
          ta.onchange = () => {
            const items = ta.value.split(/\n/).map(s=>s.trim()).filter(s=>s.length>0);
            featureExtras[m.name] = featureExtras[m.name]||{}; featureExtras[m.name][f.key] = items;
            scheduleSaveFeatureExtras();
          };
          li.appendChild(ta);
        } else if (f.type === 'route_list') {
          const ulin = document.createElement('ul'); ulin.style.listStyle='disc'; ulin.style.marginLeft='18px';
          const cur = new Set(Array.isArray(ex[f.key]) ? ex[f.key] : (f.default||[]));
          for (const r of allRoutes) {
            const rli = document.createElement('li'); rli.innerHTML = '<code>'+r+'</code>:';
            const chk = document.createElement('input'); chk.type='checkbox'; chk.className='toggle'; chk.checked = cur.has(r);
            chk.onchange = () => { if (chk.checked) cur.add(r); else cur.delete(r); featureExtras[m.name] = featureExtras[m.name]||{}; featureExtras[m.name][f.key] = Array.from(cur); scheduleSaveFeatureExtras(); };
            rli.appendChild(chk); ulin.appendChild(rli);
          }
          li.appendChild(ulin);
        } else if (f.type === 'route_number_map') {
          const ulin = document.createElement('ul'); ulin.style.listStyle='disc'; ulin.style.marginLeft='18px';
          const cur = (ex[f.key] && typeof ex[f.key]==='object') ? ex[f.key] : (f.default||{});
          for (const r of allRoutes) {
            const rli = document.createElement('li'); rli.innerHTML = '<code>'+r+'</code>:';
            const num = document.createElement('input'); num.type='number'; num.min='0'; num.placeholder='req/sec'; num.value = cur[r] ?? '';
            num.onchange = () => { const v = Number(num.value); featureExtras[m.name] = featureExtras[m.name]||{}; featureExtras[m.name][f.key] = featureExtras[m.name][f.key]||{}; if (!isFinite(v)||v<=0) { delete featureExtras[m.name][f.key][r]; } else { featureExtras[m.name][f.key][r] = Math.floor(v); } scheduleSaveFeatureExtras(); };
            rli.appendChild(num); ulin.appendChild(rli);
          }
          li.appendChild(ulin);
        }
        ul.appendChild(li);
      }
      container.appendChild(ul);
    }
    function renderFeatureTabs(manifests, groups) {
      const tabsEl = document.getElementById('feature-tabs');
      const contentTitle = document.getElementById('feature-tab-title');
      if (!tabsEl || !contentTitle) return;
      tabsEl.innerHTML = '';
      const enabledList = manifests.filter(m => enabledFeatures[m.name]);
      if (enabledList.length === 0) {
        contentTitle.textContent = 'Chưa có tính năng nào được bật';
        document.getElementById('feature-tab-content').innerHTML = '';
        return;
      }
      let activeName = tabsEl.dataset.active || enabledList[0].name;
      if (!enabledList.some(m => m.name === activeName)) activeName = enabledList[0].name;
      for (const m of enabledList) {
        const btn = document.createElement('button');
        btn.className = 'tab-btn' + (m.name===activeName ? ' active' : '');
        btn.textContent = m.name;
        btn.onclick = () => {
          tabsEl.dataset.active = m.name;
          Array.from(tabsEl.children).forEach(ch => ch.classList.remove('active'));
          btn.classList.add('active');
          contentTitle.textContent = 'Cấu hình: '+m.name;
          renderSingleFeatureSettings(m, groups);
        };
        tabsEl.appendChild(btn);
      }
      const current = enabledList.find(x => x.name === activeName) || enabledList[0];
      contentTitle.textContent = 'Cấu hình: '+current.name;
      renderSingleFeatureSettings(current, groups);
    }
    // Removed legacy OAuth2 routes UI; use dynamic feature tabs
    // Removed legacy per-route rate limit UI; use dynamic feature tabs
    function renderFeatures(s) {
      const feats = [
        ['rate_limit_enabled','Rate Limit'],
        ['waf_enabled','WAF'],
        ['oauth2_enabled','OAuth2'],
        ['admin_console_enabled','Admin Console']
      ];
      const ul = document.getElementById('feature-list');
      ul.innerHTML = '';
      for (const [key,label] of feats) {
        const li = document.createElement('li');
        const chk = document.createElement('input');
        chk.type = 'checkbox'; chk.className='toggle'; chk.checked = !!s[key];
        chk.onchange = async () => {
          await updateSettings({ [key]: chk.checked });
          enabledFeatures[label.toLowerCase().replace(/\s+/g,'_')] = chk.checked;
          await fetch('/admin/reload',{ method:'POST' });
          const routes2 = await fetchRoutes();
          window.__routes_groups = routes2;
          renderFeatureTabs(featureManifests, routes2.groups);
          // Sau khi bật tính năng, tự động mở phần cấu hình và chọn tab phù hợp
          const featName = label.toLowerCase().replace(/\s+/g,'_');
          if (chk.checked && (featureManifests||[]).some(m => m.name === featName)) {
            const tabsEl = document.getElementById('feature-tabs');
            if (tabsEl) { tabsEl.dataset.active = featName; }
            // hiển thị phần cấu hình ngay dưới Tính năng
            try { showSection('feature-config'); } catch (_) {}
          } else if (!chk.checked) {
            // nếu tắt, trở về phần Tính năng
            try { showSection('features'); } catch (_) {}
          }
        };
        li.textContent = label + ':'; li.appendChild(chk);
        ul.appendChild(li);
      }
      // Cấu hình Rate Limit theo giây
      const rli = document.createElement('li');
      rli.innerHTML = 'Rate Limit (req/s): ';
      const num = document.createElement('input');
      num.type = 'number'; num.min = '1'; num.max = '10000'; num.value = s['rate_limit_per_second'] ?? 1;
      num.onchange = async () => { await updateSettings({ rate_limit_per_second: Number(num.value) }); };
      rli.appendChild(num);
      ul.appendChild(rli);
    }
    // Bỏ UI Feature Plugins: hệ thống chỉ nạp từ thư viện build theo flags trong Features
    function renderModules(mods, disabled) {
      const ul = document.getElementById('module-list');
      ul.innerHTML='';
      for (const m of mods) {
        const li = document.createElement('li');
        const chk = document.createElement('input');
        chk.type='checkbox'; chk.className='toggle';
        chk.checked = !(disabled||[]).includes(m);
        chk.onchange = async () => {
          const cur = new Set(disabled||[]);
          if (!chk.checked) cur.add(m); else cur.delete(m);
          await updateSettings({ disabled_modules: Array.from(cur) });
        };
        li.innerHTML = '<code>'+m+'</code>:'; li.appendChild(chk);
        ul.appendChild(li);
      }
    }
    async function updateSettings(partial) {
      await fetch('/admin/settings', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify(partial) });
    }
    let saveExtrasTimer = null;
    function scheduleSaveFeatureExtras() {
      if (saveExtrasTimer) clearTimeout(saveExtrasTimer);
      saveExtrasTimer = setTimeout(async () => {
        await updateSettings({ feature_extras: featureExtras });
        await fetch('/admin/reload',{ method:'POST' });
        const routes2 = await fetchRoutes();
        window.__routes_groups = routes2;
        renderFeatureTabs(featureManifests, routes2.groups);
      }, 400);
    }
    function setActiveNav(hash) {
      const links = document.querySelectorAll('.menu a');
      links.forEach(a => { if (a.getAttribute('href') === hash) a.classList.add('active'); else a.classList.remove('active'); });
    }
    function showSection(id) {
      const ids = ['overview','features','routes','modules'];
      ids.forEach(i => { const el = document.getElementById(i); if (el) el.style.display = 'none'; });
      // reset inner toggles for features
      const flist = document.getElementById('feature-list'); if (flist) flist.style.display = '';
      const fcfg = document.getElementById('feature-config'); if (fcfg) fcfg.style.display = 'none';
      // reset inner toggles for modules
      const msec = document.getElementById('modules');
      const mlist = document.getElementById('module-list'); if (mlist) mlist.style.display = '';
      const rsec = document.getElementById('routes'); if (rsec) rsec.style.display = 'none';

      if (id === 'feature-config') {
        if (msec) msec.style.display = 'none';
        const fsec = document.getElementById('features'); if (fsec) fsec.style.display = '';
        if (flist) flist.style.display = 'none';
        if (fcfg) fcfg.style.display = '';
      } else if (id === 'routes') {
        // Hiển thị routes như mục con của modules, giữ module list để UX dễ theo dõi
        if (msec) msec.style.display = '';
        if (mlist) mlist.style.display = '';
        if (rsec) rsec.style.display = '';
      } else if (id === 'modules') {
        if (msec) msec.style.display = '';
        if (mlist) mlist.style.display = '';
        if (rsec) rsec.style.display = 'none';
      } else {
        const el = document.getElementById(id); if (el) el.style.display = '';
      }
      setActiveNav('#'+id);
    }
    function setupNavigation() {
      document.querySelectorAll('.menu a').forEach(a => {
        a.addEventListener('click', (e) => {
          e.preventDefault();
          const hash = a.getAttribute('href') || '#overview';
          const id = hash.replace('#','');
          showSection(id);
          history.replaceState(null, '', hash);
        });
      });
      const initial = (location.hash || '#overview').replace('#','');
      showSection(initial);
      window.addEventListener('hashchange', () => {
        const id = (location.hash || '#overview').replace('#','');
        showSection(id);
      });
    }
    async function init() {
      try {
        const s = await fetchSettings();
        console.log('Settings:', s);
        renderFeatures(s.settings);
        enabledFeatures.oauth2 = !!s.settings.oauth2_enabled;
        enabledFeatures.rate_limit = !!s.settings.rate_limit_enabled;
        enabledFeatures.waf = !!s.settings.waf_enabled;
        renderModules(s.modules, s.settings.disabled_modules);
        // Không hiển thị danh sách Feature Plugins nữa
        const routes = await fetchRoutes();
        console.log('Routes:', routes);
        window.__routes_groups = routes;
        disabledRoutes = s.settings.disabled_routes || [];
        featureExtras = s.settings.feature_extras || {};
        featureManifests = await fetchManifests();
        // Populate routes filter options
        try {
          const sel = document.getElementById('routes-filter');
          if (sel) {
            sel.innerHTML = '<option value="">Tất cả</option>' + (s.modules||[]).map(m => '<option value="'+m+'">'+m+'</option>').join('');
            sel.onchange = () => { const rg = window.__routes_groups || {groups:[]}; renderRoutes(rg.groups); };
          }
        } catch(e) { console.warn('Populate routes filter failed:', e); }
        renderRoutes(routes.groups);
        renderFeatureTabs(featureManifests, routes.groups);
        // Cập nhật thống kê nhanh
        try {
          const totalRoutes = (routes.groups||[]).reduce((acc,g)=>acc + (g.routes||[]).length, 0);
          const enabledModules = (s.modules||[]).filter(m => !(s.settings.disabled_modules||[]).includes(m)).length;
          const enabledFeatCount = ['rate_limit_enabled','waf_enabled','oauth2_enabled','admin_console_enabled'].filter(k => !!s.settings[k]).length;
          const setText = (id, v) => { const el = document.getElementById(id); if (el) el.textContent = String(v); };
          setText('stat-routes', totalRoutes);
          setText('stat-modules', enabledModules);
          setText('stat-features', enabledFeatCount);
          setText('ov-routes', totalRoutes);
          setText('ov-modules', enabledModules);
          setText('ov-features', enabledFeatCount);
        } catch (e) { console.warn('Stats update failed:', e); }
        const saveBtn = document.getElementById('save-routes-btn');
        if (saveBtn) {
          saveBtn.onclick = async () => {
            await fetch('/admin/routes', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({ disabled_routes: disabledRoutes }) });
            alert('Disabled routes saved');
          };
        }
        // Legacy save buttons removed; settings handled via dynamic feature tabs
        // Autosave cấu hình tính năng: không cần nút lưu
        document.getElementById('reload-btn').onclick = async () => {
          await fetch('/admin/reload',{ method:'POST' });
          const routes = await fetchRoutes();
          // re-populate filter options to reflect module changes
          try {
            const sel = document.getElementById('routes-filter');
            if (sel) {
              sel.innerHTML = '<option value="">Tất cả</option>' + (s.modules||[]).map(m => '<option value="'+m+'">'+m+'</option>').join('');
            }
          } catch(e) { console.warn('Populate routes filter after reload failed:', e); }
          renderRoutes(routes.groups);
          renderFeatureTabs(featureManifests, routes.groups);
          // cập nhật thống kê sau reload
          try {
            const totalRoutes = (routes.groups||[]).reduce((acc,g)=>acc + (g.routes||[]).length, 0);
            const setText = (id, v) => { const el = document.getElementById(id); if (el) el.textContent = String(v); };
            setText('stat-routes', totalRoutes);
            setText('ov-routes', totalRoutes);
          } catch (e) { console.warn('Stats update after reload failed:', e); }
          alert('Router reloaded');
        };
        // Thiết lập điều hướng: chỉ hiển thị nội dung tương ứng khi click
        setupNavigation();
        setupCommandPalette();
        setupGlobalSearch();
        generateSmartSuggestions();
      } catch (e) {
        console.error(e);
        const el = document.getElementById('feature-list');
        if (el) { el.innerHTML = '<li><em>'+String(e)+'</em></li>'; }
      }
    }
    window.addEventListener('DOMContentLoaded', () => { init(); });
    window.onload = () => { init(); };
  </script>
</head>
<body>
  <div class="topbar">
    <div class="brand">Admin Console</div>
    <div class="searchbar">
      <input id="global-search" type="text" placeholder="Tìm kiếm: routes/modules/features... (Enter để chuyển)" />
    </div>
    <div class="actions">
      <button id="reload-btn">Reload Router</button>
      <button onclick="openCmdPalette(true)">Command (Ctrl+K)</button>
    </div>
  </div>
  <div class="layout">
    <aside class="sidebar">
      <div class="menu">
        <a href="#overview">Tổng quan</a>
        <a href="#features">Tính năng</a>
        <a href="#feature-config" class="nav-sub">Cấu hình tính năng</a>
        <a href="#routes" class="nav-sub">Routes</a>
        <a href="#modules">Modules</a>
      </div>
      <div class="stats">
        <div><span>Routes</span><strong id="stat-routes">0</strong></div>
        <div><span>Modules</span><strong id="stat-modules">0</strong></div>
        <div><span>Features</span><strong id="stat-features">0</strong></div>
      </div>
    </aside>
    <main class="content">
      <section id="overview" class="section card">
        <h2>Tổng quan</h2>
        <div class="subtitle">Trạng thái nhanh của hệ thống</div>
        <ul>
          <li>Routes đang tải: <strong id="ov-routes">0</strong></li>
          <li>Modules bật: <strong id="ov-modules">0</strong></li>
          <li>Tính năng bật: <strong id="ov-features">0</strong></li>
        </ul>
      </section>
      <section id="features" class="section card">
        <h2>Features</h2>
        <div class="subtitle">Bật/tắt các tính năng hệ thống</div>
        <ul id="feature-list"></ul>
        <section id="feature-config" class="card nested">
          <h2>Cấu hình Tính năng</h2>
          <div class="subtitle">Chỉ hiển thị khi tính năng được bật, mỗi tính năng một tab</div>
          <div id="feature-tabs" class="tabs" data-active=""></div>
          <div class="tab-content">
            <h3 id="feature-tab-title">Chưa có tính năng nào được bật</h3>
            <div id="feature-tab-content"></div>
          </div>
        </section>
      </section>
      <section id="modules" class="section card">
        <h2>Modules</h2>
        <div class="subtitle">Bật/tắt các module hệ thống</div>
        <ul id="module-list"></ul>
        <section id="routes" class="card nested">
          <h2 style="display:flex; align-items:center; gap:8px;">Loaded Routes
            <span class="subtitle" style="margin-left:auto; display:flex; align-items:center; gap:8px;">
              <label for="routes-filter">Module:</label>
              <select id="routes-filter"><option value="">Tất cả</option></select>
              <span id="routes-count"></span>
            </span>
          </h2>
          <div class="subtitle">Danh sách route đã nạp (lọc theo module nếu cần)</div>
          <ul id="routes-list"></ul>
          <button id="save-routes-btn">Save Disabled Routes</button>
        </section>
      </section>
    </main>
  </div>
  <!-- Command Palette Overlay -->
  <div class="cmd-overlay" id="cmd-overlay">
    <div class="cmd-box">
      <h3>Command Palette</h3>
      <input id="cmd-input" class="cmd-input" placeholder="Ví dụ: bật rate limit | mở cấu hình oauth2 | lọc routes module hello" />
      <div class="cmd-hints" id="smart-hints">
        <span class="hint" data-cmd="open overview">Tới Tổng quan</span>
        <span class="hint" data-cmd="open features">Tới Tính năng</span>
        <span class="hint" data-cmd="open modules">Tới Modules</span>
        <span class="hint" data-cmd="open routes">Tới Routes</span>
      </div>
    </div>
  </div>
  <script>
    // Fallback: đảm bảo init() chạy ngay cả khi onload bị override
    (async () => { try { await init(); } catch (e) { console.error('Init error:', e); } })();
  </script>
</body>
</html>"##;
                Html(body.to_string())
            }
        }))
        .route("/settings", axum::routing::get({
            move || async move {
                let s = load_settings();
                let mut modules: Vec<String> = Vec::new();
                if let Ok(rd) = std::fs::read_dir("./modules") {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.join("Cargo.toml").exists() {
                            if let Some(name) = p.file_name().map(|s| s.to_string_lossy().to_string()) {
                                modules.push(name);
                            }
                        }
                    }
                    modules.sort();
                }
                Json(json!({ "settings": s, "modules": modules }))
            }
        }))
        .route("/settings", axum::routing::post({
            let reload_fn = reload_fn.clone();
            move |AxumJson(body): AxumJson<serde_json::Value>| async move {
                let mut s = load_settings();
                if let Some(v) = body.get("rate_limit_enabled").and_then(|v| v.as_bool()) { s.rate_limit_enabled = v; }
                if let Some(v) = body.get("rate_limit_per_second").and_then(|v| v.as_u64()) { s.rate_limit_per_second = v as u32; }
                if let Some(v) = body.get("waf_enabled").and_then(|v| v.as_bool()) { s.waf_enabled = v; }
                if let Some(v) = body.get("oauth2_enabled").and_then(|v| v.as_bool()) { s.oauth2_enabled = v; }
                if let Some(v) = body.get("admin_console_enabled").and_then(|v| v.as_bool()) { s.admin_console_enabled = v; }
                if let Some(v) = body.get("disabled_modules").and_then(|v| v.as_array()) {
                    s.disabled_modules = v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                }
                if let Some(v) = body.get("disabled_features").and_then(|v| v.as_array()) {
                    s.disabled_features = v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                    // Tự đồng bộ các cờ dựa theo tên phổ biến
                    s.waf_enabled = !s.disabled_features.iter().any(|f| f == "waf") && s.waf_enabled;
                    s.oauth2_enabled = !s.disabled_features.iter().any(|f| f == "oauth2") && s.oauth2_enabled;
                    s.rate_limit_enabled = !s.disabled_features.iter().any(|f| f == "rate_limit") && s.rate_limit_enabled;
                }
                if let Some(v) = body.get("oauth2_protected_routes").and_then(|v| v.as_array()) {
                    s.oauth2_protected_routes = v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                }
                if let Some(obj) = body.get("route_rate_limits").and_then(|v| v.as_object()) {
                    let mut map = std::collections::HashMap::new();
                    for (k, vv) in obj {
                        if let Some(n) = vv.as_u64() { map.insert(k.to_string(), n as u32); }
                    }
                    s.route_rate_limits = map;
                }
                if let Some(obj) = body.get("feature_extras").and_then(|v| v.as_object()) {
                    s.feature_extras = obj.clone();
                }
                let _ = save_settings(&s);
                // Trigger reload via provided closure
                (reload_fn)();
                Json(json!({"ok":true}))
            }
        }))
        .route("/reload", axum::routing::post({
            let reload_fn = reload_fn.clone();
            move || async move {
                (reload_fn)();
                Json(json!({"ok":true}))
            }
        }))
        .route("/routes", axum::routing::get({
            let live_spec = live_spec.clone();
            move || async move {
                let spec = live_spec.read().clone();
                let mut groups: Vec<serde_json::Value> = Vec::new();
                use std::collections::{BTreeMap, BTreeSet};
                let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

                // 1) Lấy các route từ OpenAPI (chỉ các route đang bật)
                if let Some(paths) = spec.get("paths").and_then(|v| v.as_object()) {
                    for (route, obj) in paths.iter() {
                        let prefix = route.trim_start_matches('/').split('/').next().unwrap_or("");
                        let module = obj.get("x-module").and_then(|v| v.as_str()).unwrap_or(prefix).to_string();
                        map.entry(module).or_default().insert(route.to_string());
                    }
                }

                // 2) Bổ sung các route đang bị disable để vẫn hiển thị dạng bật/tắt
                let s = load_settings();
                for route in s.disabled_routes.iter() {
                    let prefix = route.trim_start_matches('/').split('/').next().unwrap_or("");
                    let module = prefix.to_string();
                    map.entry(module).or_default().insert(route.clone());
                }

                // 3) Chuyển về dạng mảng và sắp xếp
                for (module, set) in map.into_iter() {
                    let mut routes: Vec<String> = set.into_iter().collect();
                    routes.sort();
                    groups.push(json!({"module": module, "routes": routes}));
                }

                Json(json!({"groups": groups}))
            }
        }))
        .route("/routes", axum::routing::post({
            let reload_fn = reload_fn.clone();
            move |AxumJson(body): AxumJson<serde_json::Value>| async move {
                let mut s = load_settings();
                if let Some(v) = body.get("disabled_routes").and_then(|v| v.as_array()) {
                    s.disabled_routes = v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                }
                let _ = save_settings(&s);
                // Áp dụng ngay: reload OpenAPI và router
                (reload_fn)();
                Json(json!({"ok": true}))
            }
        }))
        // Áp dụng guard IP/Host cho một số endpoint nhạy cảm
        .layer(from_fn(admin_access_guard))
}