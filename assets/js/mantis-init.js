(function () {
  try {
    if (localStorage.getItem('mantis_sidebar_collapsed') === '1') {
      document.documentElement.classList.add('sidebar-collapsed-init');
    }
  } catch (_) {}
})();
