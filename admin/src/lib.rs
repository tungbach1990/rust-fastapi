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
#[serde(default)]
pub struct FeaturesSettings {
    pub rate_limit_enabled: bool,
    pub rate_limit_per_second: u32,
    pub waf_enabled: bool,
    pub oauth2_enabled: bool,
    pub cors_enabled: bool,
    pub admin_console_enabled: bool,
    pub disabled_modules: Vec<String>,
    pub disabled_routes: Vec<String>,
    pub disabled_features: Vec<String>,
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
            cors_enabled: false,
            admin_console_enabled: true,
            disabled_modules: Vec::new(),
            disabled_routes: Vec::new(),
            disabled_features: Vec::new(),
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
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>🚀 Rust FastAPI - Admin Console</title>
  <style>
    :root {
      --bg:#0a0e1a; --panel:#111827; --panel-hover:#1f2937; --muted:#9ca3af;
      --accent:#6366f1; --accent-hover:#4f46e5; --ok:#10b981; --warn:#f59e0b; --danger:#ef4444;
      --text:#f3f4f6; --text-dim:#d1d5db; --border:#374151; --border-light:#4b5563;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body { font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--text); line-height: 1.6; }

    /* Topbar */
    .topbar { position: sticky; top:0; z-index: 50; display:flex; align-items:center; justify-content:space-between; padding: 14px 24px;
              border-bottom: 1px solid var(--border); background: rgba(17, 24, 39, 0.95); backdrop-filter: blur(12px); }
    .brand { font-size: 18px; font-weight: 700; letter-spacing: -0.5px; display: flex; align-items: center; gap: 8px; }
    .brand-icon { font-size: 24px; }
    .searchbar { flex:1; max-width: 500px; margin: 0 24px; }
    .searchbar input { width:100%; padding:10px 16px; border-radius: 12px; border: 1px solid var(--border); background: var(--panel);
                       color: var(--text); font-size: 14px; transition: all 0.2s; }
    .searchbar input:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1); }
    .actions { display:flex; gap:10px; align-items: center; }
    .btn { padding: 9px 16px; border-radius: 10px; border: 1px solid var(--border); background: var(--panel); color: var(--text);
           cursor: pointer; font-size: 14px; font-weight: 500; transition: all 0.2s; display: inline-flex; align-items: center; gap: 6px; }
    .btn:hover { border-color: var(--accent); background: var(--panel-hover); transform: translateY(-1px); }
    .btn-primary { background: var(--accent); border-color: var(--accent); color: white; }
    .btn-primary:hover { background: var(--accent-hover); }

    /* Layout */
    .layout { display:grid; grid-template-columns: 260px 1fr; gap: 20px; padding: 24px; max-width: 1800px; margin: 0 auto; }
    .sidebar { background: var(--panel); border: 1px solid var(--border); border-radius: 16px; padding: 16px;
               position: sticky; top: 84px; height: calc(100vh - 108px); overflow-y: auto; overflow-x: hidden; }
    .sidebar::-webkit-scrollbar { width: 6px; }
    .sidebar::-webkit-scrollbar-track { background: transparent; }
    .sidebar::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }

    /* Menu */
    .menu { display:flex; flex-direction:column; gap:4px; margin-bottom: 16px; }
    .menu-title { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; color: var(--muted); padding: 8px 12px; }
    .menu a { text-decoration:none; color: var(--text-dim); padding:10px 12px; border-radius:10px; font-size: 14px; font-weight: 500;
              display: flex; align-items: center; gap: 8px; transition: all 0.2s; }
    .menu a:hover { background: var(--panel-hover); color: var(--text); }
    .menu a.active { background: var(--accent); color: white; box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3); }
    .menu-icon { font-size: 16px; width: 20px; text-align: center; }

    /* Stats Cards */
    .stats { display:grid; grid-template-columns: 1fr; gap:10px; border-top: 1px solid var(--border); padding-top: 16px; margin-top: 16px; }
    .stat-card { display:flex; flex-direction: column; padding:12px; background: var(--panel-hover); border-radius:10px; border:1px solid var(--border); }
    .stat-label { font-size: 11px; font-weight: 600; text-transform: uppercase; color: var(--muted); letter-spacing: 0.5px; }
    .stat-value { font-size: 24px; font-weight: 700; color: var(--text); margin-top: 4px; }

    /* Content */
    .content { display:grid; grid-template-columns: 1fr; gap: 20px; min-width: 0; }
    .section { display: none; }
    .section.active { display: block; }
    .section-header { margin-bottom: 20px; }
    .section-title { font-size: 28px; font-weight: 700; margin-bottom: 8px; }
    .section-subtitle { font-size: 14px; color: var(--muted); }

    /* Cards */
    .card { background: var(--panel); border: 1px solid var(--border); border-radius: 16px; padding: 20px; transition: all 0.2s; }
    .card:hover { border-color: var(--border-light); }
    .card-title { font-size: 16px; font-weight: 600; margin-bottom: 16px; display: flex; align-items: center; justify-content: space-between; }
    .card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 16px; }

    /* Module/Feature Cards */
    .item-card { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 16px;
                 transition: all 0.2s; cursor: pointer; }
    .item-card:hover { border-color: var(--accent); transform: translateY(-2px); box-shadow: 0 8px 24px rgba(0,0,0,0.2); }
    .item-card-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
    .item-card-name { font-size: 16px; font-weight: 600; }
    .item-card-badge { font-size: 11px; padding: 3px 8px; border-radius: 6px; background: var(--accent); color: white; font-weight: 600; }
    .item-card-badge.disabled { background: var(--muted); }
    .item-card-desc { font-size: 13px; color: var(--muted); margin-bottom: 12px; min-height: 40px; }
    .item-card-footer { display: flex; align-items: center; justify-content: space-between; padding-top: 12px; border-top: 1px solid var(--border); }
    .item-card-meta { font-size: 12px; color: var(--muted); }

    /* Lists */
    .list { list-style: none; padding: 0; margin: 0; }
    .list-row { display: grid; grid-template-columns: 1fr auto; align-items: center; gap: 16px; padding: 12px 0;
                border-bottom: 1px solid var(--border); }
    .list-row:last-child { border-bottom: none; }
    .row-left { min-width: 0; }
    .row-right { text-align: right; }

    /* Toggle Switch */
    .switch { position: relative; display:inline-block; width: 48px; height: 26px; vertical-align: middle; }
    .switch input { opacity: 0; width: 0; height: 0; }
    .slider { position:absolute; cursor:pointer; top:0; left:0; right:0; bottom:0; background: var(--border); transition: .3s; border-radius: 26px; }
    .slider:before { position:absolute; content:""; height:20px; width:20px; left:3px; bottom:3px; background:white; transition:.3s; border-radius:50%; }
    input:checked + .slider { background: var(--accent); }
    input:checked + .slider:before { transform: translateX(22px); }

    /* Command Palette */
    .cmd-overlay { position: fixed; inset:0; background: rgba(0,0,0,0.7); backdrop-filter: blur(4px); display:none;
                   align-items: flex-start; justify-content: center; z-index: 100; padding-top: 15vh; }
    .cmd-box { width: 600px; max-width: 90vw; background: var(--panel); border: 1px solid var(--border); border-radius: 16px;
               padding: 20px; box-shadow: 0 20px 60px rgba(0,0,0,0.5); }
    .cmd-box h3 { margin:0 0 12px 0; font-size: 18px; }
    .cmd-input { width: 100%; padding: 12px 16px; border-radius: 12px; border: 1px solid var(--border); background: var(--bg);
                 color: var(--text); font-size: 15px; }
    .cmd-input:focus { outline: none; border-color: var(--accent); }
    .cmd-hints { display:flex; flex-wrap:wrap; gap:8px; margin-top:12px; }
    .hint { padding:6px 12px; border:1px solid var(--border); border-radius: 8px; font-size:12px; color: var(--muted);
            cursor:pointer; transition: all 0.2s; }
    .hint:hover { color: var(--text); border-color: var(--accent); background: var(--panel-hover); }

    /* Tabs */
    .tabs { display: flex; gap: 8px; border-bottom: 1px solid var(--border); margin-bottom: 16px; }
    .tab-btn { padding: 10px 16px; border-radius: 10px 10px 0 0; border: 1px solid transparent; background: transparent;
               color: var(--muted); cursor: pointer; font-size: 14px; font-weight: 500; transition: all 0.2s; }
    .tab-btn:hover { color: var(--text); background: var(--panel-hover); }
    .tab-btn.active { background: var(--panel); border-color: var(--border); border-bottom-color: var(--panel); color: var(--text); }

    /* Utilities */
    code { background: var(--bg); padding: 3px 8px; border-radius: 6px; border: 1px solid var(--border); font-size: 13px; font-family: 'Fira Code', monospace; }
    .badge { font-size: 11px; padding: 3px 8px; border-radius: 6px; font-weight: 600; margin-left: 8px; }
    .badge-success { background: var(--ok); color: white; }
    .badge-warning { background: var(--warn); color: white; }
    .badge-danger { background: var(--danger); color: white; }
    input[type="number"], input[type="text"], textarea, select {
      padding: 8px 12px; border-radius: 8px; border: 1px solid var(--border); background: var(--bg); color: var(--text); font-size: 14px;
    }
    input:focus, textarea:focus, select:focus { outline: none; border-color: var(--accent); }
    textarea { width: 100%; font-family: 'Fira Code', monospace; resize: vertical; }

    /* Settings Table */
    .settings-table { width: 100%; border-collapse: collapse; margin-top: 12px; }
    .settings-table td { padding: 12px 8px; border-bottom: 1px solid var(--border); vertical-align: middle; }
    .settings-table td:first-child { font-weight: 500; width: 200px; }
    .settings-table td:last-child { text-align: right; }
    .settings-table tr:last-child td { border-bottom: none; }

    /* Responsive */
    @media (max-width: 1024px) {
      .layout { grid-template-columns: 1fr; }
      .sidebar { position: relative; height: auto; top:0; }
      .card-grid { grid-template-columns: 1fr; }
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
    let enabledFeatures = { oauth2:false, rate_limit:false, waf:false, cors:false };
    // Toggle helpers
    function ensureToggleStyles() {
      if (document.getElementById('toggle-styles')) return;
      const style = document.createElement('style');
      style.id = 'toggle-styles';
      style.textContent = `
        .switch { position: relative; display:inline-block; width: 44px; height: 24px; vertical-align: middle; }
        .switch input { opacity: 0; width: 0; height: 0; }
        .slider { position:absolute; cursor:pointer; top:0; left:0; right:0; bottom:0; background:#23304d; transition: .2s; border-radius: 16px; }
        .slider:before { position:absolute; content:""; height:18px; width:18px; left:3px; bottom:3px; background:#a8b3c9; transition:.2s; border-radius:50%; }
        input:checked + .slider { background:#4f46e5; }
        input:checked + .slider:before { transform: translateX(20px); background:#e2e8f0; }
      `;
      document.head.appendChild(style);
    }
    function makeToggle(defaultOn, onChange) {
      ensureToggleStyles();
      const label = document.createElement('label');
      label.className = 'switch';
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.checked = !!defaultOn;
      const slider = document.createElement('span');
      slider.className = 'slider';
      input.addEventListener('change', async () => {
        try { await onChange(!!input.checked); } catch(e) { console.error('toggle change failed', e); input.checked = !input.checked; }
      });
      label.appendChild(input);
      label.appendChild(slider);
      return { root: label, input };
    }
    // Global search
    function setupGlobalSearch() {
      const input = document.getElementById('global-search');
      if (!input) return;
      input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
          const q = (input.value||'').trim().toLowerCase();
          if (q.startsWith('modules')) { showSection('modules'); return; }
          if (q.startsWith('features')) { showSection('features'); return; }
          // quick filter routes by module name
          if (q.length>0) {
            const rg = window.__routes_groups || {groups:[]};
            const found = rg.groups.find(g => g.module.toLowerCase().includes(q));
            if (found) {
              showSection('modules');
              const tabsEl = document.getElementById('module-routes-tabs');
              if (tabsEl) tabsEl.dataset.active = found.module;
              renderModuleRoutesTabs(rg.groups);
            }
          }
        }
      });
    }
    function renderSingleModuleRoutes(moduleName, routes) {
      const container = document.getElementById('module-routes-tab-content');
      if (!container) return;
      container.innerHTML = '';

      if (!routes || routes.length === 0) {
        container.innerHTML = '<p style="color: var(--muted); padding: 16px;">Không có routes nào trong module này</p>';
        return;
      }

      // Add Enable All / Disable All buttons
      const btnContainer = document.createElement('div');
      btnContainer.style.cssText = 'display: flex; gap: 8px; margin-bottom: 12px;';

      const enableAllBtn = document.createElement('button');
      enableAllBtn.className = 'btn btn-primary';
      enableAllBtn.textContent = '✅ Enable All Routes';
      enableAllBtn.onclick = async () => {
        const cur = new Set(disabledRoutes||[]);
        for (const r of routes) {
          cur.delete(r);
        }
        disabledRoutes = Array.from(cur);
        await fetch('/admin/routes', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({ disabled_routes: disabledRoutes }) });
        // Re-render to update toggles
        renderSingleModuleRoutes(moduleName, routes);
      };

      const disableAllBtn = document.createElement('button');
      disableAllBtn.className = 'btn';
      disableAllBtn.textContent = '❌ Disable All Routes';
      disableAllBtn.onclick = async () => {
        const cur = new Set(disabledRoutes||[]);
        for (const r of routes) {
          cur.add(r);
        }
        disabledRoutes = Array.from(cur);
        await fetch('/admin/routes', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({ disabled_routes: disabledRoutes }) });
        // Re-render to update toggles
        renderSingleModuleRoutes(moduleName, routes);
      };

      btnContainer.appendChild(enableAllBtn);
      btnContainer.appendChild(disableAllBtn);
      container.appendChild(btnContainer);

      const table = document.createElement('table');
      table.className = 'settings-table';

      for (const r of routes) {
        const tr = document.createElement('tr');
        const td1 = document.createElement('td');
        td1.innerHTML = '<code>'+r+'</code>';
        const td2 = document.createElement('td');

        const enabled = !(disabledRoutes||[]).includes(r);
        const tg = makeToggle(enabled, async (on) => {
          const cur = new Set(disabledRoutes||[]);
          if (!on) cur.add(r); else cur.delete(r);
          disabledRoutes = Array.from(cur);
          // Auto-save disabled routes
          await fetch('/admin/routes', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({ disabled_routes: disabledRoutes }) });
        });
        td2.appendChild(tg.root);
        tr.appendChild(td1);
        tr.appendChild(td2);
        table.appendChild(tr);
      }

      container.appendChild(table);
    }

    function renderModuleRoutesTabs(groups) {
      const tabsEl = document.getElementById('module-routes-tabs');
      const contentTitle = document.getElementById('module-routes-tab-title');
      if (!tabsEl || !contentTitle) return;

      tabsEl.innerHTML = '';

      if (!groups || groups.length === 0) {
        contentTitle.textContent = 'Không có module nào';
        document.getElementById('module-routes-tab-content').innerHTML = '';
        return;
      }

      let activeName = tabsEl.dataset.active || groups[0].module;
      if (!groups.some(g => g.module === activeName)) activeName = groups[0].module;

      for (const g of groups) {
        const btn = document.createElement('button');
        btn.className = 'tab-btn' + (g.module===activeName ? ' active' : '');
        btn.textContent = g.module;
        btn.onclick = () => {
          tabsEl.dataset.active = g.module;
          Array.from(tabsEl.children).forEach(ch => ch.classList.remove('active'));
          btn.classList.add('active');
          contentTitle.textContent = 'Routes: '+g.module + ' (' + g.routes.length + ')';
          renderSingleModuleRoutes(g.module, g.routes);
        };
        tabsEl.appendChild(btn);
      }

      const current = groups.find(x => x.module === activeName) || groups[0];
      contentTitle.textContent = 'Routes: '+current.module + ' (' + current.routes.length + ')';
      renderSingleModuleRoutes(current.module, current.routes);
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
        const rg = window.__routes_groups || {groups:[]};
        const found = rg.groups.find(g => g.module === cmd.target);
        if (found) {
          showSection('modules');
          const tabsEl = document.getElementById('module-routes-tabs');
          if (tabsEl) tabsEl.dataset.active = found.module;
          renderModuleRoutesTabs(rg.groups);
        }
        return;
      }
      if (cmd.action==='enable-feature' || cmd.action==='disable-feature') {
        const feat = cmd.target; const on = cmd.action==='enable-feature';
        // Update both *_enabled flag and disabled_features for consistency
        const s = await fetchSettings();
        const cur = new Set(s.settings.disabled_features || []);
        if (!on) cur.add(feat); else cur.delete(feat);
        const updates = { disabled_features: Array.from(cur) };
        // Also update specific *_enabled flag if it exists
        const enabledFlagKey = feat + '_enabled';
        if (s.settings[enabledFlagKey] !== undefined) {
          updates[enabledFlagKey] = on;
        }
        await updateSettings(updates);
        enabledFeatures[feat] = on;
        await fetch('/admin/reload',{ method:'POST' });
        // Reload manifests to ensure new feature tabs appear
        featureManifests = await fetchManifests();
        const routes2 = await fetchRoutes(); window.__routes_groups = routes2;
        renderModuleRoutesTabs(routes2.groups);
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
          // Add Enable All / Disable All buttons for route_list
          const btnContainer = document.createElement('div');
          btnContainer.style.cssText = 'display: flex; gap: 8px; margin-bottom: 12px; margin-top: 8px;';

          const enableAllBtn = document.createElement('button');
          enableAllBtn.className = 'btn btn-primary';
          enableAllBtn.textContent = '✅ Enable All';
          enableAllBtn.onclick = () => {
            const cur = new Set(allRoutes);
            featureExtras[m.name] = featureExtras[m.name]||{};
            featureExtras[m.name][f.key] = Array.from(cur);
            scheduleSaveFeatureExtras();
            // Re-render to update toggles
            renderSingleFeatureSettings(m, groups);
          };

          const disableAllBtn = document.createElement('button');
          disableAllBtn.className = 'btn';
          disableAllBtn.textContent = '❌ Disable All';
          disableAllBtn.onclick = () => {
            featureExtras[m.name] = featureExtras[m.name]||{};
            featureExtras[m.name][f.key] = [];
            scheduleSaveFeatureExtras();
            // Re-render to update toggles
            renderSingleFeatureSettings(m, groups);
          };

          btnContainer.appendChild(enableAllBtn);
          btnContainer.appendChild(disableAllBtn);
          li.appendChild(btnContainer);

          const table = document.createElement('table');
          table.className = 'settings-table';
          const cur = new Set(Array.isArray(ex[f.key]) ? ex[f.key] : (f.default||[]));
          for (const r of allRoutes) {
            const tr = document.createElement('tr');
            const td1 = document.createElement('td');
            td1.innerHTML = '<code>'+r+'</code>';
            const td2 = document.createElement('td');
            const tg = makeToggle(cur.has(r), (on) => {
              if (on) cur.add(r); else cur.delete(r);
              featureExtras[m.name] = featureExtras[m.name]||{};
              featureExtras[m.name][f.key] = Array.from(cur);
              scheduleSaveFeatureExtras();
            });
            td2.appendChild(tg.root);
            tr.appendChild(td1);
            tr.appendChild(td2);
            table.appendChild(tr);
          }
          li.appendChild(table);
        } else if (f.type === 'route_number_map') {
          // Add Clear All button for route_number_map
          const btnContainer = document.createElement('div');
          btnContainer.style.cssText = 'display: flex; gap: 8px; margin-bottom: 12px; margin-top: 8px;';

          const clearAllBtn = document.createElement('button');
          clearAllBtn.className = 'btn';
          clearAllBtn.textContent = '🗑️ Clear All';
          clearAllBtn.onclick = () => {
            featureExtras[m.name] = featureExtras[m.name]||{};
            featureExtras[m.name][f.key] = {};
            scheduleSaveFeatureExtras();
            // Re-render to update inputs
            renderSingleFeatureSettings(m, groups);
          };

          btnContainer.appendChild(clearAllBtn);
          li.appendChild(btnContainer);

          const table = document.createElement('table');
          table.className = 'settings-table';
          const cur = (ex[f.key] && typeof ex[f.key]==='object') ? ex[f.key] : (f.default||{});
          for (const r of allRoutes) {
            const tr = document.createElement('tr');
            const td1 = document.createElement('td');
            td1.innerHTML = '<code>'+r+'</code>';
            const td2 = document.createElement('td');
            const num = document.createElement('input');
            num.type='number'; num.min='0'; num.placeholder='req/sec'; num.value = cur[r] ?? '';
            num.style.width = '120px';
            num.onchange = () => {
              const v = Number(num.value);
              featureExtras[m.name] = featureExtras[m.name]||{};
              featureExtras[m.name][f.key] = featureExtras[m.name][f.key]||{};
              if (!isFinite(v)||v<=0) { delete featureExtras[m.name][f.key][r]; }
              else { featureExtras[m.name][f.key][r] = Math.floor(v); }
              scheduleSaveFeatureExtras();
            };
            td2.appendChild(num);
            tr.appendChild(td1);
            tr.appendChild(td2);
            table.appendChild(tr);
          }
          li.appendChild(table);
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
      ensureToggleStyles();
      const container = document.getElementById('feature-list');
      container.innerHTML = '';
      const disabled = (s && s.disabled_features) || [];

      for (const m of (featureManifests || [])) {
        // Use same logic as init() to determine if feature is enabled
        const notDisabled = !disabled.includes(m.name);
        const enabledFlagKey = m.name + '_enabled';
        const hasEnabledFlag = s[enabledFlagKey] !== undefined;
        const isEnabled = hasEnabledFlag ? (notDisabled && !!s[enabledFlagKey]) : notDisabled;
        const card = document.createElement('div');
        card.className = 'item-card';

        // Header
        const header = document.createElement('div');
        header.className = 'item-card-header';
        const name = document.createElement('div');
        name.className = 'item-card-name';
        name.textContent = m.label || m.name || 'Feature';
        const badge = document.createElement('span');
        badge.className = 'item-card-badge ' + (isEnabled ? '' : 'disabled');
        badge.textContent = isEnabled ? 'ENABLED' : 'DISABLED';
        header.appendChild(name);
        header.appendChild(badge);

        // Description
        const desc = document.createElement('div');
        desc.className = 'item-card-desc';
        desc.textContent = m.description || `Feature plugin: ${m.name}`;

        // Footer with toggle
        const footer = document.createElement('div');
        footer.className = 'item-card-footer';
        const meta = document.createElement('div');
        meta.className = 'item-card-meta';
        meta.textContent = m.version ? `v${m.version}` : 'Plugin';

        const tg = makeToggle(isEnabled, async (on) => {
          const cur = new Set(((await fetchSettings()).disabled_features) || disabled);
          if (!on) cur.add(m.name); else cur.delete(m.name);
          // Update both disabled_features and specific *_enabled flag if it exists
          const updates = { disabled_features: Array.from(cur) };
          const enabledFlagKey = m.name + '_enabled';
          // Check if this feature has a specific *_enabled flag
          const s = await fetchSettings();
          if (s.settings[enabledFlagKey] !== undefined) {
            updates[enabledFlagKey] = on;
          }
          await updateSettings(updates);
          enabledFeatures[m.name] = on;
          await fetch('/admin/reload', { method: 'POST' });
          // Reload manifests to ensure new feature tabs appear
          featureManifests = await fetchManifests();
          const routes2 = await fetchRoutes();
          window.__routes_groups = routes2;
          renderModuleRoutesTabs(routes2.groups);
          renderFeatureTabs(featureManifests, routes2.groups);

          // Update UI
          badge.textContent = on ? 'ENABLED' : 'DISABLED';
          badge.className = 'item-card-badge ' + (on ? '' : 'disabled');

          // Update active tab if feature was just enabled
          if (on) {
            const tabsEl = document.getElementById('feature-tabs');
            if (tabsEl) tabsEl.dataset.active = m.name;
          }

          // Update stats
          try {
            const enabledCount = Object.values(enabledFeatures).filter(Boolean).length;
            const sf = document.getElementById('stat-features'); if (sf) sf.textContent = String(enabledCount);
            const ovf = document.getElementById('ov-features'); if (ovf) ovf.textContent = String(enabledCount);
          } catch(_) {}
        });

        footer.appendChild(meta);
        footer.appendChild(tg.root);

        card.appendChild(header);
        card.appendChild(desc);
        card.appendChild(footer);
        container.appendChild(card);
      }
    }
    function renderModules(mods, disabled) {
      const container = document.getElementById('module-list');
      container.innerHTML='';

      for (const m of mods) {
        const isEnabled = !(disabled||[]).includes(m);
        const card = document.createElement('div');
        card.className = 'item-card';

        // Header
        const header = document.createElement('div');
        header.className = 'item-card-header';
        const name = document.createElement('div');
        name.className = 'item-card-name';
        name.textContent = m;
        const badge = document.createElement('span');
        badge.className = 'item-card-badge ' + (isEnabled ? '' : 'disabled');
        badge.textContent = isEnabled ? 'ACTIVE' : 'DISABLED';
        header.appendChild(name);
        header.appendChild(badge);

        // Description
        const desc = document.createElement('div');
        desc.className = 'item-card-desc';
        desc.textContent = `Module plugin từ thư mục modules/${m}`;

        // Footer with toggle
        const footer = document.createElement('div');
        footer.className = 'item-card-footer';
        const meta = document.createElement('div');
        meta.className = 'item-card-meta';
        meta.textContent = '📦 Module';

        const tg = makeToggle(isEnabled, async (on) => {
          const cur = new Set(disabled||[]);
          if (!on) cur.add(m); else cur.delete(m);
          await updateSettings({ disabled_modules: Array.from(cur) });

          // Reload router and update routes tabs
          await fetch('/admin/reload', { method: 'POST' });
          const routes2 = await fetchRoutes();
          window.__routes_groups = routes2;
          renderModuleRoutesTabs(routes2.groups);
          renderFeatureTabs(featureManifests, routes2.groups);

          // Update UI
          badge.textContent = on ? 'ACTIVE' : 'DISABLED';
          badge.className = 'item-card-badge ' + (on ? '' : 'disabled');

          // Update stats
          try {
            const s = await fetchSettings();
            const enabledModules = (s.modules||[]).filter(mod => !(s.settings.disabled_modules||[]).includes(mod)).length;
            const sm = document.getElementById('stat-modules'); if (sm) sm.textContent = String(enabledModules);
            const ovm = document.getElementById('ov-modules'); if (ovm) ovm.textContent = String(enabledModules);
          } catch(_) {}
        });

        footer.appendChild(meta);
        footer.appendChild(tg.root);

        card.appendChild(header);
        card.appendChild(desc);
        card.appendChild(footer);
        container.appendChild(card);
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
        // Reload manifests to ensure tabs are up to date
        featureManifests = await fetchManifests();
        const routes2 = await fetchRoutes();
        window.__routes_groups = routes2;
        renderModuleRoutesTabs(routes2.groups);
        renderFeatureTabs(featureManifests, routes2.groups);
      }, 400);
    }
    function setActiveNav(hash) {
      const links = document.querySelectorAll('.menu a');
      links.forEach(a => { if (a.getAttribute('href') === hash) a.classList.add('active'); else a.classList.remove('active'); });
    }
    function showSection(id) {
      // Hide all sections
      const sections = document.querySelectorAll('.section');
      sections.forEach(s => s.classList.remove('active'));

      // Show selected section
      const target = document.getElementById(id);
      if (target) target.classList.add('active');

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
    function setupBulkActions(s) {
      // Enable All Modules
      const enableAllModulesBtn = document.getElementById('enable-all-modules-btn');
      if (enableAllModulesBtn) {
        enableAllModulesBtn.onclick = async () => {
          await updateSettings({ disabled_modules: [] });
          await fetch('/admin/reload', { method: 'POST' });
          const routes2 = await fetchRoutes();
          window.__routes_groups = routes2;
          renderModuleRoutesTabs(routes2.groups);
          renderFeatureTabs(featureManifests, routes2.groups);
          // Re-render modules list
          const s2 = await fetchSettings();
          renderModules(s2.modules, s2.settings.disabled_modules);
          // Update stats
          const enabledModules = s2.modules.length;
          const sm = document.getElementById('stat-modules'); if (sm) sm.textContent = String(enabledModules);
          const ovm = document.getElementById('ov-modules'); if (ovm) ovm.textContent = String(enabledModules);
        };
      }

      // Disable All Modules
      const disableAllModulesBtn = document.getElementById('disable-all-modules-btn');
      if (disableAllModulesBtn) {
        disableAllModulesBtn.onclick = async () => {
          const allModules = s.modules || [];
          await updateSettings({ disabled_modules: allModules });
          await fetch('/admin/reload', { method: 'POST' });
          const routes2 = await fetchRoutes();
          window.__routes_groups = routes2;
          renderModuleRoutesTabs(routes2.groups);
          renderFeatureTabs(featureManifests, routes2.groups);
          // Re-render modules list
          const s2 = await fetchSettings();
          renderModules(s2.modules, s2.settings.disabled_modules);
          // Update stats
          const sm = document.getElementById('stat-modules'); if (sm) sm.textContent = '0';
          const ovm = document.getElementById('ov-modules'); if (ovm) ovm.textContent = '0';
        };
      }

      // Enable All Features
      const enableAllFeaturesBtn = document.getElementById('enable-all-features-btn');
      if (enableAllFeaturesBtn) {
        enableAllFeaturesBtn.onclick = async () => {
          const updates = { disabled_features: [] };
          // Also enable all *_enabled flags
          for (const m of featureManifests) {
            const enabledFlagKey = m.name + '_enabled';
            const s2 = await fetchSettings();
            if (s2.settings[enabledFlagKey] !== undefined) {
              updates[enabledFlagKey] = true;
            }
          }
          await updateSettings(updates);
          // Update enabledFeatures
          for (const m of featureManifests) {
            enabledFeatures[m.name] = true;
          }
          await fetch('/admin/reload', { method: 'POST' });
          featureManifests = await fetchManifests();
          const routes2 = await fetchRoutes();
          window.__routes_groups = routes2;
          renderModuleRoutesTabs(routes2.groups);
          renderFeatureTabs(featureManifests, routes2.groups);
          // Re-render features list
          const s3 = await fetchSettings();
          renderFeatures(s3.settings);
          // Update stats
          const enabledCount = featureManifests.length;
          const sf = document.getElementById('stat-features'); if (sf) sf.textContent = String(enabledCount);
          const ovf = document.getElementById('ov-features'); if (ovf) ovf.textContent = String(enabledCount);
        };
      }

      // Disable All Features
      const disableAllFeaturesBtn = document.getElementById('disable-all-features-btn');
      if (disableAllFeaturesBtn) {
        disableAllFeaturesBtn.onclick = async () => {
          const allFeatures = featureManifests.map(m => m.name);
          const updates = { disabled_features: allFeatures };
          // Also disable all *_enabled flags
          for (const m of featureManifests) {
            const enabledFlagKey = m.name + '_enabled';
            const s2 = await fetchSettings();
            if (s2.settings[enabledFlagKey] !== undefined) {
              updates[enabledFlagKey] = false;
            }
          }
          await updateSettings(updates);
          // Update enabledFeatures
          for (const m of featureManifests) {
            enabledFeatures[m.name] = false;
          }
          await fetch('/admin/reload', { method: 'POST' });
          featureManifests = await fetchManifests();
          const routes2 = await fetchRoutes();
          window.__routes_groups = routes2;
          renderModuleRoutesTabs(routes2.groups);
          renderFeatureTabs(featureManifests, routes2.groups);
          // Re-render features list
          const s3 = await fetchSettings();
          renderFeatures(s3.settings);
          // Update stats
          const sf = document.getElementById('stat-features'); if (sf) sf.textContent = '0';
          const ovf = document.getElementById('ov-features'); if (ovf) ovf.textContent = '0';
        };
      }
    }
    async function init() {
      try {
        const s = await fetchSettings();
        console.log('Settings:', s);
        featureManifests = await fetchManifests();
        // Populate enabledFeatures from manifests and settings
        // A feature is enabled if: NOT in disabled_features AND *_enabled flag is true (if exists)
        const disabled = s.settings.disabled_features || [];
        for (const m of featureManifests) {
          const notDisabled = !disabled.includes(m.name);
          // Check if there's a specific *_enabled flag for this feature
          const enabledFlagKey = m.name + '_enabled';
          const hasEnabledFlag = s.settings[enabledFlagKey] !== undefined;
          if (hasEnabledFlag) {
            // If there's a specific flag, use it (but still respect disabled_features)
            enabledFeatures[m.name] = notDisabled && !!s.settings[enabledFlagKey];
          } else {
            // No specific flag, just use disabled_features
            enabledFeatures[m.name] = notDisabled;
          }
        }
        renderModules(s.modules, s.settings.disabled_modules);
        // Không hiển thị danh sách Feature Plugins nữa
        const routes = await fetchRoutes();
        console.log('Routes:', routes);
        window.__routes_groups = routes;
        disabledRoutes = s.settings.disabled_routes || [];
        featureExtras = s.settings.feature_extras || {};
        renderFeatures(s.settings);
        renderModuleRoutesTabs(routes.groups);
        renderFeatureTabs(featureManifests, routes.groups);
        // Cập nhật thống kê nhanh
        try {
          const totalRoutes = (routes.groups||[]).reduce((acc,g)=>acc + (g.routes||[]).length, 0);
          const enabledModules = (s.modules||[]).filter(m => !(s.settings.disabled_modules||[]).includes(m)).length;
          const enabledCount = Object.values(enabledFeatures).filter(Boolean).length;
          const sr = document.getElementById('stat-routes'); if (sr) sr.textContent = String(totalRoutes);
          const sm = document.getElementById('stat-modules'); if (sm) sm.textContent = String(enabledModules);
          const sf = document.getElementById('stat-features'); if (sf) sf.textContent = String(enabledCount);
          const ovr = document.getElementById('ov-routes'); if (ovr) ovr.textContent = String(totalRoutes);
          const ovm = document.getElementById('ov-modules'); if (ovm) ovm.textContent = String(enabledModules);
          const ovf = document.getElementById('ov-features'); if (ovf) ovf.textContent = String(enabledCount);
        } catch(_) {}

        // Thiết lập điều hướng: chỉ hiển thị nội dung tương ứng khi click
        setupNavigation();
        setupCommandPalette();
        setupGlobalSearch();
        generateSmartSuggestions();
        setupBulkActions(s);
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
    <div class="brand">
      <span class="brand-icon">🚀</span>
      <span>Rust FastAPI Admin</span>
    </div>
    <div class="searchbar">
      <input id="global-search" type="text" placeholder="🔍 Tìm kiếm modules, features, routes... (Enter)" />
    </div>
    <div class="actions">
      <button onclick="openCmdPalette(true)" class="btn btn-primary">⌘ Command</button>
    </div>
  </div>
  <div class="layout">
    <aside class="sidebar">
      <div class="menu-title">Navigation</div>
      <div class="menu">
        <a href="#overview"><span class="menu-icon">📊</span> Dashboard</a>
        <a href="#modules"><span class="menu-icon">📦</span> Modules</a>
        <a href="#features"><span class="menu-icon">⚡</span> Features</a>
      </div>
      <div class="stats">
        <div class="stat-card">
          <div class="stat-label">Total Routes</div>
          <div class="stat-value" id="stat-routes">0</div>
        </div>
        <div class="stat-card">
          <div class="stat-label">Active Modules</div>
          <div class="stat-value" id="stat-modules">0</div>
        </div>
        <div class="stat-card">
          <div class="stat-label">Enabled Features</div>
          <div class="stat-value" id="stat-features">0</div>
        </div>
      </div>
    </aside>
    <main class="content">
      <!-- Dashboard Overview -->
      <section id="overview" class="section">
        <div class="section-header">
          <h1 class="section-title">📊 Dashboard</h1>
          <p class="section-subtitle">Tổng quan hệ thống Rust FastAPI</p>
        </div>
        <div class="card-grid">
          <div class="card">
            <div class="card-title">🛣️ Routes</div>
            <div style="font-size: 36px; font-weight: 700; color: var(--accent);" id="ov-routes">0</div>
            <div class="section-subtitle">Total API endpoints</div>
          </div>
          <div class="card">
            <div class="card-title">📦 Modules</div>
            <div style="font-size: 36px; font-weight: 700; color: var(--ok);" id="ov-modules">0</div>
            <div class="section-subtitle">Active modules</div>
          </div>
          <div class="card">
            <div class="card-title">⚡ Features</div>
            <div style="font-size: 36px; font-weight: 700; color: var(--warn);" id="ov-features">0</div>
            <div class="section-subtitle">Enabled features</div>
          </div>
        </div>
      </section>

      <!-- Modules Section -->
      <section id="modules" class="section">
        <div class="section-header">
          <h1 class="section-title">📦 Modules</h1>
          <p class="section-subtitle">Quản lý các module độc lập trong thư mục modules/</p>
          <div style="display: flex; gap: 8px; margin-top: 12px;">
            <button id="enable-all-modules-btn" class="btn btn-primary">✅ Enable All</button>
            <button id="disable-all-modules-btn" class="btn">❌ Disable All</button>
          </div>
        </div>
        <div id="module-list" class="card-grid"></div>

        <!-- Module Routes Configuration -->
        <div id="module-routes-config" style="margin-top: 24px;">
          <div class="card">
            <div class="card-title">
              <span>🛣️ Routes theo Module</span>
            </div>
            <div id="module-routes-tabs" class="tabs" data-active=""></div>
            <div class="tab-content">
              <h3 id="module-routes-tab-title">Chọn module để xem routes</h3>
              <div id="module-routes-tab-content"></div>
            </div>
          </div>
        </div>
      </section>

      <!-- Features Section -->
      <section id="features" class="section">
        <div class="section-header">
          <h1 class="section-title">⚡ Features</h1>
          <p class="section-subtitle">Quản lý các tính năng trong thư mục features/</p>
          <div style="display: flex; gap: 8px; margin-top: 12px;">
            <button id="enable-all-features-btn" class="btn btn-primary">✅ Enable All</button>
            <button id="disable-all-features-btn" class="btn">❌ Disable All</button>
          </div>
        </div>
        <div id="feature-list" class="card-grid"></div>

        <!-- Feature Configuration -->
        <div id="feature-config" style="margin-top: 24px;">
          <div class="card">
            <div class="card-title">⚙️ Cấu hình Features</div>
            <div id="feature-tabs" class="tabs" data-active=""></div>
            <div class="tab-content">
              <h3 id="feature-tab-title">Chưa có tính năng nào được bật</h3>
              <div id="feature-tab-content"></div>
            </div>
          </div>
        </div>
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