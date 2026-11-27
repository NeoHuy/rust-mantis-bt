(function () {
  'use strict';

  var SIDEBAR_KEY = 'mantis_sidebar_collapsed';
  var RECENT_PREFIX = 'mantis_recently_visited_';
  var WIDGET_PREFIX = 'mantis_widget_collapsed_';

  function getCsrfToken() {
    var meta = document.querySelector('meta[name="csrf-token"]');
    return meta ? meta.getAttribute('content') || '' : '';
  }

  function closeAllDropdowns(except) {
    var openMenus = document.querySelectorAll('.nav-dropdown.open');
    for (var i = 0; i < openMenus.length; i++) {
      if (openMenus[i] !== except) {
        openMenus[i].classList.remove('open');
        var btn = openMenus[i].querySelector('[aria-expanded]');
        if (btn) btn.setAttribute('aria-expanded', 'false');
      }
    }
  }

  function initDropdowns() {
    var toggles = document.querySelectorAll('[data-dropdown-toggle]');
    for (var i = 0; i < toggles.length; i++) {
      (function (btn) {
        btn.addEventListener('click', function (e) {
          e.preventDefault();
          e.stopPropagation();
          var container = btn.closest('.nav-dropdown');
          if (!container) return;
          var willOpen = !container.classList.contains('open');
          closeAllDropdowns(container);
          if (willOpen) {
            container.classList.add('open');
            btn.setAttribute('aria-expanded', 'true');
            var searchInput = container.querySelector('[data-project-search]');
            if (searchInput) {
              searchInput.value = '';
              filterProjects(container, '');
              setTimeout(function () { searchInput.focus(); }, 0);
            }
          } else {
            container.classList.remove('open');
            btn.setAttribute('aria-expanded', 'false');
          }
        });
      })(toggles[i]);
    }

    document.addEventListener('click', function (e) {
      if (!e.target.closest('.nav-dropdown')) {
        closeAllDropdowns(null);
      }
    });
  }

  function filterProjects(container, query) {
    var q = query.trim().toLowerCase();
    var items = container.querySelectorAll('[data-project-id]');
    for (var i = 0; i < items.length; i++) {
      var label = (items[i].getAttribute('data-project-name') || items[i].textContent || '').toLowerCase();
      if (!q || label.indexOf(q) !== -1) {
        items[i].removeAttribute('hidden');
      } else {
        items[i].setAttribute('hidden', '');
      }
    }
  }

  function initProjectSwitcher() {
    var searchInputs = document.querySelectorAll('[data-project-search]');
    for (var i = 0; i < searchInputs.length; i++) {
      (function (input) {
        input.addEventListener('input', function () {
          var container = input.closest('.nav-dropdown');
          if (container) filterProjects(container, input.value);
        });
        input.addEventListener('click', function (e) {
          e.stopPropagation();
        });
      })(searchInputs[i]);
    }

    var options = document.querySelectorAll('[data-project-id]');
    for (var j = 0; j < options.length; j++) {
      (function (opt) {
        opt.addEventListener('click', function (e) {
          e.preventDefault();
          var pid = opt.getAttribute('data-project-id');
          if (pid === null) return;
          opt.disabled = true;
          fetch('/set_project', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/x-www-form-urlencoded',
              'X-CSRF-Token': getCsrfToken()
            },
            body: 'project_id=' + encodeURIComponent(pid)
          })
            .then(function (res) {
              if (res && res.ok) {
                window.location.reload();
              } else {
                opt.disabled = false;
              }
            })
            .catch(function () {
              opt.disabled = false;
            });
        });
      })(options[j]);
    }
  }

  function applySidebarCollapsed(sidebar, icon, collapsed) {
    if (!sidebar) return;
    document.documentElement.classList.remove('sidebar-collapsed-init');
    if (collapsed) {
      sidebar.classList.add('menu-min');
      if (icon) {
        icon.classList.remove('fa-angle-double-left');
        icon.classList.add('fa-angle-double-right');
      }
    } else {
      sidebar.classList.remove('menu-min');
      if (icon) {
        icon.classList.remove('fa-angle-double-right');
        icon.classList.add('fa-angle-double-left');
      }
    }
  }

  function initSidebar() {
    var sidebar = document.getElementById('sidebar');
    var collapseBtn = document.getElementById('sidebar-collapse-btn');
    var collapseIcon = collapseBtn ? collapseBtn.querySelector('i') : null;
    var menuToggler = document.getElementById('menu-toggler');
    var backdrop = document.getElementById('sidebar-backdrop');

    if (!sidebar) return;

    try {
      if (window.localStorage.getItem(SIDEBAR_KEY) === '1') {
        applySidebarCollapsed(sidebar, collapseIcon, true);
      } else {
        document.documentElement.classList.remove('sidebar-collapsed-init');
      }
    } catch (_) {
      document.documentElement.classList.remove('sidebar-collapsed-init');
    }

    if (collapseBtn) {
      collapseBtn.addEventListener('click', function () {
        var nextCollapsed = !sidebar.classList.contains('menu-min');
        applySidebarCollapsed(sidebar, collapseIcon, nextCollapsed);
        try {
          window.localStorage.setItem(SIDEBAR_KEY, nextCollapsed ? '1' : '0');
        } catch (_) {}
      });
    }

    function setMobileDrawer(open, restoreFocus) {
      if (open) {
        sidebar.classList.add('mobile-open');
        if (backdrop) backdrop.classList.add('open');
        if (menuToggler) menuToggler.setAttribute('aria-expanded', 'true');
        var firstLink = sidebar.querySelector('a');
        if (firstLink) firstLink.focus();
      } else {
        var wasOpen = sidebar.classList.contains('mobile-open');
        sidebar.classList.remove('mobile-open');
        if (backdrop) backdrop.classList.remove('open');
        if (menuToggler) {
          menuToggler.setAttribute('aria-expanded', 'false');
          if (restoreFocus && wasOpen) menuToggler.focus();
        }
      }
    }

    if (menuToggler) {
      menuToggler.addEventListener('click', function () {
        setMobileDrawer(!sidebar.classList.contains('mobile-open'), false);
      });
    }

    if (backdrop) {
      backdrop.addEventListener('click', function () {
        setMobileDrawer(false, true);
      });
    }

    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape') {
        var activeInDropdown = document.activeElement
          ? document.activeElement.closest('.nav-dropdown.open')
          : null;
        var dropdownToggle = activeInDropdown
          ? activeInDropdown.querySelector('[data-dropdown-toggle]')
          : null;
        closeAllDropdowns(null);
        if (dropdownToggle) dropdownToggle.focus();
        setMobileDrawer(false, true);
      }
    });
  }

  function initJumpToIssue() {
    var form = document.getElementById('nav-search-form');
    var input = document.getElementById('jump_bug_id');
    if (!input) return;

    function handleJump(e) {
      var raw = input.value.trim();
      if (/^\d+$/.test(raw)) {
        var v = parseInt(raw, 10);
        if (v > 0) {
          if (e) e.preventDefault();
          window.location.href = '/view_bug_page/' + v;
          return;
        }
      }
      if (!raw && e) {
        e.preventDefault();
      }
    }

    if (form) {
      form.addEventListener('submit', handleJump);
    }

    input.addEventListener('keydown', function (e) {
      if (e.key === 'Enter') {
        handleJump(e);
      }
    });
  }

  function initRecentlyVisited() {
    var container = document.getElementById('nav-recent');
    var username = document.body ? document.body.getAttribute('data-username') || 'anon' : 'anon';
    var storageKey = RECENT_PREFIX + username;
    var match = window.location.pathname.match(/^\/view_bug_page\/(\d+)$/);
    var isIssuePage = document.title && document.title.indexOf('Issue #') === 0;
    var list = [];

    try {
      var raw = window.localStorage.getItem(storageKey);
      if (raw) {
        var parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          list = parsed.filter(function (id) {
            return typeof id === 'number' && Number.isInteger(id) && id > 0;
          });
        }
      }
      if (match && isIssuePage) {
        var currentId = parseInt(match[1], 10);
        if (currentId > 0) {
          list = [currentId].concat(list.filter(function (id) { return id !== currentId; })).slice(0, 5);
          window.localStorage.setItem(storageKey, JSON.stringify(list));
        }
      }
    } catch (_) {
      return;
    }

    if (!container || list.length === 0) return;

    container.textContent = '';
    container.appendChild(document.createTextNode('Recently Visited: '));

    for (var i = 0; i < list.length; i++) {
      if (i > 0) {
        container.appendChild(document.createTextNode(', '));
      }
      var idNum = list[i];
      var link = document.createElement('a');
      link.href = '/view_bug_page/' + String(idNum);
      var padded = String(idNum);
      while (padded.length < 7) padded = '0' + padded;
      link.textContent = padded;
      container.appendChild(link);
    }

    container.removeAttribute('hidden');
  }

  function applyWidgetCollapsed(box, btn, collapsed) {
    var icon = btn ? btn.querySelector('i') : null;
    if (collapsed) {
      box.classList.add('collapsed');
      if (icon) {
        icon.classList.remove('fa-chevron-up');
        icon.classList.add('fa-chevron-down');
      }
      if (btn) btn.setAttribute('aria-expanded', 'false');
    } else {
      box.classList.remove('collapsed');
      if (icon) {
        icon.classList.remove('fa-chevron-down');
        icon.classList.add('fa-chevron-up');
      }
      if (btn) btn.setAttribute('aria-expanded', 'true');
    }
  }

  function expandWidgetForElement(targetEl) {
    if (!targetEl) return;
    var cur = targetEl;
    while (cur && cur !== document.body) {
      var box = cur.classList && cur.classList.contains('widget-box')
        ? cur
        : cur.closest('.widget-box');
      if (!box) break;
      if (box.classList.contains('collapsed')) {
        var btn = box.querySelector('[data-action="collapse"]');
        applyWidgetCollapsed(box, btn, false);
        var widgetId = box.getAttribute('data-widget-id');
        if (widgetId) {
          try {
            window.localStorage.setItem(WIDGET_PREFIX + widgetId, '0');
          } catch (_) {}
        }
      }
      cur = box.parentElement;
    }
  }

  function expandForHash(hashStr) {
    if (!hashStr || hashStr.length < 2) return;
    var rawId = hashStr.charAt(0) === '#' ? hashStr.slice(1) : hashStr;
    if (!rawId) return;
    var target = null;
    try {
      target = document.getElementById(decodeURIComponent(rawId));
    } catch (_) {
      target = document.getElementById(rawId);
    }
    if (target) {
      expandWidgetForElement(target);
    }
  }

  function initWidgetBoxes() {
    var triggers = document.querySelectorAll('.widget-box [data-action="collapse"]');
    for (var i = 0; i < triggers.length; i++) {
      (function (btn) {
        var box = btn.closest('.widget-box');
        if (!box) return;
        var widgetId = box.getAttribute('data-widget-id');

        if (widgetId) {
          try {
            var saved = window.localStorage.getItem(WIDGET_PREFIX + widgetId);
            if (saved === '1') applyWidgetCollapsed(box, btn, true);
            else if (saved === '0') applyWidgetCollapsed(box, btn, false);
          } catch (_) {}
        }

        btn.addEventListener('click', function (e) {
          e.preventDefault();
          var next = !box.classList.contains('collapsed');
          applyWidgetCollapsed(box, btn, next);
          if (widgetId) {
            try {
              window.localStorage.setItem(WIDGET_PREFIX + widgetId, next ? '1' : '0');
            } catch (_) {}
          }
        });
      })(triggers[i]);
    }

    if (window.location.hash) {
      expandForHash(window.location.hash);
    }
    window.addEventListener('hashchange', function () {
      expandForHash(window.location.hash);
    });
    document.addEventListener('click', function (e) {
      var link = e.target && e.target.closest ? e.target.closest('a[href^="#"]') : null;
      if (link) {
        expandForHash(link.getAttribute('href'));
      }
    });
  }

  function initSelectAllCheckboxes() {
    var selectAll = document.getElementById('select_all_bugs');
    if (!selectAll) return;
    var items = document.querySelectorAll('input[name="bug_arr"]');

    selectAll.addEventListener('change', function () {
      for (var i = 0; i < items.length; i++) {
        items[i].checked = selectAll.checked;
      }
      selectAll.indeterminate = false;
    });

    for (var j = 0; j < items.length; j++) {
      items[j].addEventListener('change', function () {
        var checkedCount = 0;
        for (var k = 0; k < items.length; k++) {
          if (items[k].checked) checkedCount++;
        }
        if (items.length === 0) {
          selectAll.checked = false;
          selectAll.indeterminate = false;
        } else if (checkedCount === 0) {
          selectAll.checked = false;
          selectAll.indeterminate = false;
        } else if (checkedCount === items.length) {
          selectAll.checked = true;
          selectAll.indeterminate = false;
        } else {
          selectAll.checked = false;
          selectAll.indeterminate = true;
        }
      });
    }
  }

  function initSavedFilterQuickSelect() {
    var sel = document.getElementById('saved_filter_select');
    if (!sel) return;
    sel.addEventListener('change', function () {
      var val = sel.value;
      if (val && val !== '0') {
        window.location.href = '/view_all_bug_page?filter_id=' + encodeURIComponent(val);
      }
    });
  }

  function initConfirmButtons() {
    document.addEventListener('submit', function (e) {
      if (e.defaultPrevented) return;
      var form = e.target;
      if (!form || !form.getAttribute) return;
      if (form.hasAttribute('data-confirm')) {
        var confirmMsg = form.getAttribute('data-confirm') || 'Are you sure?';
        if (!window.confirm(confirmMsg)) {
          e.preventDefault();
          return;
        }
      }
      if (form.getAttribute('data-disable-on-submit') === 'true') {
        var btn = form.querySelector('button[type="submit"]');
        if (btn) {
          btn.disabled = true;
        }
      }
    });

    document.addEventListener('click', function (e) {
      if (e.defaultPrevented) return;
      var el = e.target && e.target.closest ? e.target.closest('[data-confirm]') : null;
      if (!el || el.tagName === 'FORM') return;
      var msg = el.getAttribute('data-confirm') || 'Are you sure?';
      if (!window.confirm(msg)) {
        e.preventDefault();
      }
    });
  }

  function initCountdownRedirect() {
    var containers = document.querySelectorAll('[data-redirect-url]');
    for (var i = 0; i < containers.length; i++) {
      (function (container) {
        var url = (container.getAttribute('data-redirect-url') || '').trim();
        if (!url || url.charAt(0) !== '/' || url.charAt(1) === '/' || url.charAt(1) === '\\') {
          return;
        }
        var rawSecs = parseInt(container.getAttribute('data-redirect-seconds') || '5', 10);
        var seconds = isNaN(rawSecs) || rawSecs < 1 ? 5 : rawSecs;
        var countdownEl = container.querySelector('[data-redirect-countdown]');
        if (countdownEl) {
          countdownEl.textContent = String(seconds);
        }
        var interval = window.setInterval(function () {
          seconds -= 1;
          if (countdownEl) {
            countdownEl.textContent = String(seconds);
          }
          if (seconds <= 0) {
            window.clearInterval(interval);
            window.location.href = url;
          }
        }, 1000);
      })(containers[i]);
    }
  }

  function initDropzones() {
    var zones = document.querySelectorAll('[data-dropzone]');
    for (var i = 0; i < zones.length; i++) {
      (function (dz) {
        var inputId = dz.getAttribute('data-dropzone-input');
        var listId = dz.getAttribute('data-dropzone-list');
        var inp = inputId ? document.getElementById(inputId) : dz.querySelector('input[type="file"]');
        var fl = listId ? document.getElementById(listId) : null;
        if (!inp) return;

        function updateList() {
          if (!fl) return;
          var files = inp.files;
          if (!files || files.length === 0) {
            fl.textContent = '';
            return;
          }
          var names = [];
          for (var j = 0; j < files.length; j++) {
            names.push(files[j].name + ' (' + Math.round(files[j].size / 1024) + ' KB)');
          }
          fl.textContent = 'Selected: ' + names.join(', ');
        }

        dz.addEventListener('click', function (e) {
          if (e.target !== inp) {
            inp.click();
          }
        });
        dz.addEventListener('dragover', function (e) {
          e.preventDefault();
          dz.classList.add('is-dragover');
        });
        dz.addEventListener('dragleave', function (e) {
          e.preventDefault();
          dz.classList.remove('is-dragover');
        });
        dz.addEventListener('drop', function (e) {
          e.preventDefault();
          dz.classList.remove('is-dragover');
          if (e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files.length > 0) {
            inp.files = e.dataTransfer.files;
            updateList();
          }
        });
        inp.addEventListener('change', updateList);
      })(zones[i]);
    }
  }

  function initBulkActionForm() {
    var form = document.getElementById('bulk_action_form');
    var sel = document.getElementById('bulk_action_select');
    if (!sel) return;

    function updateBulkActionInputs(act) {
      var groups = document.querySelectorAll('.bulk-input-group');
      for (var i = 0; i < groups.length; i++) {
        var grp = groups[i];
        var acts = grp.getAttribute('data-actions') || '';
        if (acts.split(',').indexOf(act) !== -1) {
          grp.style.display = 'inline-flex';
        } else {
          grp.style.display = 'none';
        }
      }
    }

    updateBulkActionInputs(sel.value);
    sel.addEventListener('change', function () {
      updateBulkActionInputs(sel.value);
    });

    if (form) {
      form.addEventListener('submit', function (e) {
        if (!sel.value) {
          window.alert('Please select an action.');
          e.preventDefault();
          return;
        }
        var checked = document.querySelectorAll('input[name="bug_arr"]:checked');
        if (checked.length === 0) {
          window.alert('Please select at least one issue.');
          e.preventDefault();
          return;
        }
        if (sel.value === 'DELETE') {
          if (!window.confirm('Are you sure you want to permanently delete ' + checked.length + ' selected issue(s)?')) {
            e.preventDefault();
          }
        }
      });
    }
  }

  function initCopyButtons() {
    var btns = document.querySelectorAll('[data-copy-target]');
    for (var i = 0; i < btns.length; i++) {
      (function (btn) {
        btn.addEventListener('click', function () {
          var targetId = btn.getAttribute('data-copy-target');
          var el = targetId ? document.getElementById(targetId) : null;
          if (!el) return;
          var val = el.value !== undefined ? el.value : (el.textContent || '');
          if (val && val.charAt(0) === '/') {
            val = window.location.origin + val;
          }
          var origHtml = btn.innerHTML;
          function showCopied() {
            btn.textContent = 'Copied!';
            setTimeout(function () {
              btn.innerHTML = origHtml;
            }, 1500);
          }
          if (navigator.clipboard && navigator.clipboard.writeText) {
            navigator.clipboard.writeText(val).then(showCopied).catch(function () {
              var ok = false;
              if (el.select) {
                el.select();
                try { ok = document.execCommand('copy'); } catch (_) {}
              }
              if (ok) showCopied();
            });
          } else {
            var ok = false;
            if (el.select) {
              el.select();
              try { ok = document.execCommand('copy'); } catch (_) {}
            }
            if (ok) showCopied();
          }
        });
      })(btns[i]);
    }
  }

  function initPrintButtons() {
    var btns = document.querySelectorAll('[data-action="print"]');
    for (var i = 0; i < btns.length; i++) {
      btns[i].addEventListener('click', function () {
        window.print();
      });
    }
  }

  function initProjectReloadSelects() {
    var selects = document.querySelectorAll('select[data-reload-url]');
    for (var i = 0; i < selects.length; i++) {
      (function (sel) {
        sel.addEventListener('change', function () {
          var base = sel.getAttribute('data-reload-url');
          if (!base) return;
          var sep = base.indexOf('?') === -1 ? '?' : '&';
          window.location.assign(base + sep + 'project_id=' + encodeURIComponent(sel.value));
        });
      })(selects[i]);
    }
  }

  function initDisableOnSubmit() {
    var forms = document.querySelectorAll('form[data-disable-on-submit]');
    for (var i = 0; i < forms.length; i++) {
      (function (form) {
        form.addEventListener('submit', function () {
          var btn = form.querySelector('button[type="submit"], input[type="submit"]');
          if (btn) btn.disabled = true;
        });
      })(forms[i]);
    }
  }

  window.MantisBT = window.MantisBT || {};
  window.MantisBT.getCsrfToken = getCsrfToken;

  document.addEventListener('DOMContentLoaded', function () {
    initDropdowns();
    initProjectSwitcher();
    initProjectReloadSelects();
    initSidebar();
    initJumpToIssue();
    initRecentlyVisited();
    initWidgetBoxes();
    initSelectAllCheckboxes();
    initSavedFilterQuickSelect();
    initConfirmButtons();
    initDisableOnSubmit();
    initCountdownRedirect();
    initDropzones();
    initBulkActionForm();
    initCopyButtons();
    initPrintButtons();
  });
})();
