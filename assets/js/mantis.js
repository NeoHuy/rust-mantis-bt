(function () {
  'use strict';

  var SIDEBAR_KEY = 'mantis_sidebar_collapsed';
  var RECENT_KEY = 'mantis_recently_visited';
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
            .then(function () { window.location.reload(); })
            .catch(function () { window.location.reload(); });
        });
      })(options[j]);
    }
  }

  function applySidebarCollapsed(sidebar, icon, collapsed) {
    if (!sidebar) return;
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
      }
    } catch (_) {}

    if (collapseBtn) {
      collapseBtn.addEventListener('click', function () {
        var nextCollapsed = !sidebar.classList.contains('menu-min');
        applySidebarCollapsed(sidebar, collapseIcon, nextCollapsed);
        try {
          window.localStorage.setItem(SIDEBAR_KEY, nextCollapsed ? '1' : '0');
        } catch (_) {}
      });
    }

    function setMobileDrawer(open) {
      if (open) {
        sidebar.classList.add('mobile-open');
        if (backdrop) backdrop.classList.add('open');
        if (menuToggler) menuToggler.setAttribute('aria-expanded', 'true');
      } else {
        sidebar.classList.remove('mobile-open');
        if (backdrop) backdrop.classList.remove('open');
        if (menuToggler) menuToggler.setAttribute('aria-expanded', 'false');
      }
    }

    if (menuToggler) {
      menuToggler.addEventListener('click', function () {
        setMobileDrawer(!sidebar.classList.contains('mobile-open'));
      });
    }

    if (backdrop) {
      backdrop.addEventListener('click', function () {
        setMobileDrawer(false);
      });
    }

    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape') {
        closeAllDropdowns(null);
        setMobileDrawer(false);
      }
    });
  }

  function initJumpToIssue() {
    var input = document.getElementById('jump_bug_id');
    if (!input) return;
    input.addEventListener('keydown', function (e) {
      if (e.key === 'Enter') {
        e.preventDefault();
        var v = parseInt(input.value, 10);
        if (v > 0) {
          window.location.href = '/view_bug_page/' + v;
        }
      }
    });
  }

  function initRecentlyVisited() {
    var container = document.getElementById('nav-recent');
    var match = window.location.pathname.match(/^\/view_bug_page\/(\d+)$/);
    var list = [];
    try {
      var raw = window.localStorage.getItem(RECENT_KEY);
      if (raw) {
        var parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          list = parsed.filter(function (id) {
            return typeof id === 'number' && Number.isInteger(id) && id > 0;
          });
        }
      }
      if (match) {
        var currentId = parseInt(match[1], 10);
        if (currentId > 0) {
          list = [currentId].concat(list.filter(function (id) { return id !== currentId; })).slice(0, 5);
          window.localStorage.setItem(RECENT_KEY, JSON.stringify(list));
        }
      }
    } catch (_) {
      return;
    }

    if (!container || list.length === 0) return;

    container.textContent = '';
    var prefix = document.createTextNode('Recently Visited: ');
    container.appendChild(prefix);

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

  function initWidgetBoxes() {
    var triggers = document.querySelectorAll('.widget-box [data-action="collapse"]');
    for (var i = 0; i < triggers.length; i++) {
      (function (btn) {
        var box = btn.closest('.widget-box');
        if (!box) return;
        var widgetId = box.getAttribute('data-widget-id');
        var icon = btn.querySelector('i');

        function setCollapsed(collapsed) {
          if (collapsed) {
            box.classList.add('collapsed');
            if (icon) {
              icon.classList.remove('fa-chevron-up');
              icon.classList.add('fa-chevron-down');
            }
            btn.setAttribute('aria-expanded', 'false');
          } else {
            box.classList.remove('collapsed');
            if (icon) {
              icon.classList.remove('fa-chevron-down');
              icon.classList.add('fa-chevron-up');
            }
            btn.setAttribute('aria-expanded', 'true');
          }
        }

        if (widgetId) {
          try {
            var saved = window.localStorage.getItem(WIDGET_PREFIX + widgetId);
            if (saved === '1') setCollapsed(true);
            else if (saved === '0') setCollapsed(false);
          } catch (_) {}
        }

        btn.addEventListener('click', function (e) {
          e.preventDefault();
          var next = !box.classList.contains('collapsed');
          setCollapsed(next);
          if (widgetId) {
            try {
              window.localStorage.setItem(WIDGET_PREFIX + widgetId, next ? '1' : '0');
            } catch (_) {}
          }
        });
      })(triggers[i]);
    }
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

  window.MantisBT = window.MantisBT || {};
  window.MantisBT.getCsrfToken = getCsrfToken;

  document.addEventListener('DOMContentLoaded', function () {
    initDropdowns();
    initProjectSwitcher();
    initSidebar();
    initJumpToIssue();
    initRecentlyVisited();
    initWidgetBoxes();
    initSelectAllCheckboxes();
    initSavedFilterQuickSelect();
  });
})();
