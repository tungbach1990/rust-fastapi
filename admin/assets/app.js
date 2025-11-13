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