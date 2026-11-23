// Google Analytics 4 for this site. GA_ID is the one place to set the measurement ID.
(function () {
  var GA_ID = 'G-ZZPYXGLN2H';
  if (!/^G-[A-Z0-9]+$/.test(GA_ID) || GA_ID === 'G-SAMHQPLACEHOLDER') return;
  var h = location.hostname;
  window.dataLayer = window.dataLayer || [];
  window.gtag = function () { dataLayer.push(arguments); };
  gtag('js', new Date());
  // Every *.samhq.app site shares one cookie on the root domain; GA tells them apart by hostname.
  gtag('config', GA_ID, { anonymize_ip: true, cookie_domain: /(^|\.)samhq\.app$/.test(h) ? 'samhq.app' : 'auto' });
  var s = document.createElement('script');
  s.async = true;
  s.src = 'https://www.googletagmanager.com/gtag/js?id=' + GA_ID;
  document.head.appendChild(s);
})();
