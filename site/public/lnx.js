// The Linux install on an Ubuntu 24.04 desktop (GNOME 46, Yaru, light windows): Firefox downloads the
// AppImage, a double-click in Files opens Hushpen, and Hushpen types the first dictation into Text Editor.
// The scene sits straight in the page, a rounded desktop with a plain list of the three steps under it. The
// desktop has a fixed logical size; a camera scales it into its stage and follows the action so the windows
// stay readable. It plays only while seen. With Reduce Motion it shows its last frame.
(() => {
  const host = document.querySelector('#linux .ln-v');
  if (!host || !window.FlowBar) return;
  const calm = FlowBar.calm;
  const EASE = 'cubic-bezier(.22,1,.36,1)';
  // A hand speeds up and slows down; the page's ease-out would start the pointer with a jump
  const HAND = 'cubic-bezier(.45,.05,.25,1)';
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  let FILE = 'Hushpen.AppImage', bytes = 0;
  const size = n => window.HushpenReleases.size(n);
  const SAID = 'Hello from Ubuntu. This is my first dictation, and it never left my computer.';
  const STEPS = [['Download', 'Get the AppImage. One file.'], ['Double-click', 'Open it from your Downloads folder.'], ['Talk', 'Double tap <kbd>Right Alt</kbd> and speak. It types for you.']];
  const ASSETS = ['linux-arm64-appimage', 'linux-arm64-deb', 'linux-arm64-rpm', 'linux-arm64-tar', 'linux-x64-appimage', 'linux-x64-deb', 'linux-x64-rpm', 'linux-x64-tar', 'macos-dmg', 'macos-zip'];
  const PROG = [.13, .33, .57, .8];

  // GNOME and Firefox symbolic icons on a 16 pt grid
  const G = {
    back: '<path d="M10 3.5 5.5 8l4.5 4.5"/>', fwd: '<path d="M6 3.5 10.5 8 6 12.5"/>',
    reload: '<path d="M12.5 8a4.5 4.5 0 1 1-1.4-3.3"/><path d="M12.6 2.6v2.9H9.7"/>',
    dl: '<path d="M8 2.5v7.6M4.9 7.2 8 10.3l3.1-3.1M3 13.3h10"/>', menu: '<path d="M2.5 4h11M2.5 8h11M2.5 12h11"/>',
    lock: '<rect x="3.5" y="7" width="9" height="6.5" rx="1.5"/><path d="M5.5 7V5.5a2.5 2.5 0 0 1 5 0V7"/>',
    min: '<path d="M4.5 10.5h7"/>', max: '<rect x="4.5" y="4.5" width="7" height="7" rx="1.2"/>', x: '<path d="m4.8 4.8 6.4 6.4M11.2 4.8l-6.4 6.4"/>',
    plus: '<path d="M8 3.5v9M3.5 8h9"/>', chev: '<path d="m4.5 6.2 3.5 3.5 3.5-3.5"/>', tri: '<path d="M4.5 6h7L8 10.5z" fill="currentColor" stroke="none"/>',
    search: '<circle cx="7" cy="7" r="4.3"/><path d="m10.3 10.3 3.2 3.2"/>',
    grid: '<rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1"/><rect x="9" y="2.5" width="4.5" height="4.5" rx="1"/><rect x="2.5" y="9" width="4.5" height="4.5" rx="1"/><rect x="9" y="9" width="4.5" height="4.5" rx="1"/>',
    dots: '<circle cx="8" cy="3.6" r="1.1" fill="currentColor" stroke="none"/><circle cx="8" cy="8" r="1.1" fill="currentColor" stroke="none"/><circle cx="8" cy="12.4" r="1.1" fill="currentColor" stroke="none"/>',
    fxv: '<rect x="2.3" y="3" width="11.4" height="10" rx="2"/><path d="M6.2 3v10"/>',
    acct: '<circle cx="8" cy="8" r="5.8"/><circle cx="8" cy="6.7" r="2"/><path d="M4.4 12.2a4.4 4.4 0 0 1 7.2 0"/>',
    recent: '<circle cx="8" cy="8" r="5.6"/><path d="M8 5v3.2l2.1 1.3"/>',
    star: '<path d="m8 2.4 1.7 3.5 3.8.6-2.8 2.6.7 3.8L8 11.1 4.6 12.9l.7-3.8-2.8-2.6 3.8-.6z"/>',
    home: '<path d="M2.6 7.3 8 2.9l5.4 4.4"/><path d="M4 6.3V13h8V6.3"/>',
    desk: '<rect x="2.3" y="3" width="11.4" height="8" rx="1.3"/><path d="M5.8 13.5h4.4M8 11v2.5"/>',
    doc: '<path d="M4 2.5h5.2L12 5.3v8.2H4z"/><path d="M9 2.5v3h3"/>', down: '<path d="M8 2.5v7.2M5 7l3 3 3-3M3 13h10"/>',
    music: '<path d="M6 11.8V3.6l7-1.4v8.2"/><circle cx="4.4" cy="11.8" r="1.6"/><circle cx="11.4" cy="10.4" r="1.6"/>',
    pic: '<rect x="2.5" y="3.5" width="11" height="9" rx="1.5"/><path d="m3 11.2 3.4-3.4 2.4 2.4 1.6-1.6 2.6 2.6"/>',
    video: '<rect x="2" y="4" width="8.6" height="8" rx="1.5"/><path d="m10.6 7 3.4-2v6l-3.4-2"/>',
    trash: '<path d="M3 4.6h10M6.4 4.6V3h3.2v1.6M4.4 4.6l.7 8.4h5.8l.7-8.4"/>',
    wifi: '<path d="M1.8 6.2a9 9 0 0 1 12.4 0M4 8.6a5.8 5.8 0 0 1 8 0M6.2 11a2.6 2.6 0 0 1 3.6 0"/>',
    vol: '<path d="M2.5 6h2.3L8 3.4v9.2L4.8 10H2.5z"/><path d="M10.5 5.8a3.2 3.2 0 0 1 0 4.4M12.3 4a5.8 5.8 0 0 1 0 8"/>',
    bat: '<rect x="1.5" y="5" width="11.5" height="6" rx="1.6"/><path d="M14.5 7v2"/><rect x="3" y="6.5" width="7" height="3" rx=".6" fill="currentColor" stroke="none"/>',
  };
  // The Octicons github.com draws on the release page, as in its own markup (16 px grid)
  const OC = {
    pkg: '<path d="m8.878.392 5.25 3.045c.54.314.872.89.872 1.514v6.098a1.75 1.75 0 0 1-.872 1.514l-5.25 3.045a1.75 1.75 0 0 1-1.756 0l-5.25-3.045A1.75 1.75 0 0 1 1 11.049V4.951c0-.624.332-1.201.872-1.514L7.122.392a1.75 1.75 0 0 1 1.756 0ZM7.875 1.69l-4.63 2.685L8 7.133l4.755-2.758-4.63-2.685a.248.248 0 0 0-.25 0ZM2.5 5.677v5.372c0 .09.047.171.125.216l4.625 2.683V8.432Zm6.25 8.271 4.625-2.683a.25.25 0 0 0 .125-.216V5.677L8.75 8.432Z"/>',
    zip: '<path d="M3.5 1.75v11.5c0 .09.048.173.126.217a.75.75 0 0 1-.752 1.298A1.748 1.748 0 0 1 2 13.25V1.75C2 .784 2.784 0 3.75 0h5.586c.464 0 .909.185 1.237.513l2.914 2.914c.329.328.513.773.513 1.237v8.586A1.75 1.75 0 0 1 12.25 15h-.5a.75.75 0 0 1 0-1.5h.5a.25.25 0 0 0 .25-.25V4.664a.25.25 0 0 0-.073-.177L9.513 1.573a.25.25 0 0 0-.177-.073H7.25a.75.75 0 0 1 0 1.5h-.5a.75.75 0 0 1 0-1.5h-3a.25.25 0 0 0-.25.25Zm3.75 8.75h.5c.966 0 1.75.784 1.75 1.75v3a.75.75 0 0 1-.75.75h-2.5a.75.75 0 0 1-.75-.75v-3c0-.966.784-1.75 1.75-1.75ZM6 5.25a.75.75 0 0 1 .75-.75h.5a.75.75 0 0 1 0 1.5h-.5A.75.75 0 0 1 6 5.25Zm.75 2.25h.5a.75.75 0 0 1 0 1.5h-.5a.75.75 0 0 1 0-1.5ZM8 6.75A.75.75 0 0 1 8.75 6h.5a.75.75 0 0 1 0 1.5h-.5A.75.75 0 0 1 8 6.75ZM8.75 3h.5a.75.75 0 0 1 0 1.5h-.5a.75.75 0 0 1 0-1.5ZM8 9.75A.75.75 0 0 1 8.75 9h.5a.75.75 0 0 1 0 1.5h-.5A.75.75 0 0 1 8 9.75Zm-1 2.5v2.25h1v-2.25a.25.25 0 0 0-.25-.25h-.5a.25.25 0 0 0-.25.25Z"/>',
    copy: '<path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z"/><path d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"/>',
    person: '<path d="M10.561 8.073a6.005 6.005 0 0 1 3.432 5.142.75.75 0 1 1-1.498.07 4.5 4.5 0 0 0-8.99 0 .75.75 0 0 1-1.498-.07 6.004 6.004 0 0 1 3.431-5.142 3.999 3.999 0 1 1 5.123 0ZM10.5 5a2.5 2.5 0 1 0-5 0 2.5 2.5 0 0 0 5 0Z"/>',
    bars: '<path d="M1 2.75A.75.75 0 0 1 1.75 2h12.5a.75.75 0 0 1 0 1.5H1.75A.75.75 0 0 1 1 2.75Zm0 5A.75.75 0 0 1 1.75 7h12.5a.75.75 0 0 1 0 1.5H1.75A.75.75 0 0 1 1 7.75ZM1.75 12h12.5a.75.75 0 0 1 0 1.5H1.75a.75.75 0 0 1 0-1.5Z"/>',
    tri: '<path d="m4.427 7.427 3.396 3.396a.25.25 0 0 0 .354 0l3.396-3.396A.25.25 0 0 0 11.396 7H4.604a.25.25 0 0 0-.177.427Z"/>',
    tag: '<path d="M1 7.775V2.75C1 1.784 1.784 1 2.75 1h5.025c.464 0 .91.184 1.238.513l6.25 6.25a1.75 1.75 0 0 1 0 2.474l-5.026 5.026a1.75 1.75 0 0 1-2.474 0l-6.25-6.25A1.752 1.752 0 0 1 1 7.775Zm1.5 0c0 .066.026.13.073.177l6.25 6.25a.25.25 0 0 0 .354 0l5.025-5.025a.25.25 0 0 0 0-.354l-6.25-6.25a.25.25 0 0 0-.177-.073H2.75a.25.25 0 0 0-.25.25ZM6 5a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z"/>',
    commit: '<path d="M11.93 8.5a4.002 4.002 0 0 1-7.86 0H.75a.75.75 0 0 1 0-1.5h3.32a4.002 4.002 0 0 1 7.86 0h3.32a.75.75 0 0 1 0 1.5Zm-1.43-.75a2.5 2.5 0 1 0-5 0 2.5 2.5 0 0 0 5 0Z"/>'
  };
  const oc = (k, c = '') => `<svg class="oc${c ? ' ' + c : ''}" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">${OC[k]}</svg>`;
  const MKP = 'M10.226 17.284c-2.965-.36-5.054-2.493-5.054-5.256 0-1.123.404-2.336 1.078-3.144-.292-.741-.247-2.314.09-2.965.898-.112 2.111.36 2.83 1.01.853-.269 1.752-.404 2.853-.404 1.1 0 1.999.135 2.807.382.696-.629 1.932-1.1 2.83-.988.315.606.36 2.179.067 2.942.72.854 1.101 2 1.101 3.167 0 2.763-2.089 4.852-5.098 5.234.763.494 1.28 1.572 1.28 2.807v2.336c0 .674.561 1.056 1.235.786 4.066-1.55 7.255-5.615 7.255-10.646C23.5 6.188 18.334 1 11.978 1 5.62 1 .5 6.188.5 12.545c0 4.986 3.167 9.12 7.435 10.669.606.225 1.19-.18 1.19-.786V20.63a2.9 2.9 0 0 1-1.078.224c-1.483 0-2.359-.808-2.987-2.313-.247-.607-.517-.966-1.034-1.033-.27-.023-.359-.135-.359-.27 0-.27.45-.471.898-.471.652 0 1.213.404 1.797 1.235.45.651.921.943 1.483.943.561 0 .92-.202 1.437-.719.382-.381.674-.718.944-.943';
  const GH24 = `<svg class="rl-mk" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"><path d="${MKP}"/></svg>`;
  // The GitHub mark as github.com draws it now
  const GHM = c => `<svg class="${c}" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M6.766 11.328c-2.063-.25-3.516-1.734-3.516-3.656 0-.781.281-1.625.75-2.188-.203-.515-.172-1.609.063-2.062.625-.078 1.468.25 1.968.703.594-.187 1.219-.281 1.985-.281.765 0 1.39.094 1.953.265.484-.437 1.344-.765 1.969-.687.218.422.25 1.515.046 2.047.5.593.766 1.39.766 2.203 0 1.922-1.453 3.375-3.547 3.64.531.344.89 1.094.89 1.954v1.625c0 .468.391.734.86.547C13.781 14.359 16 11.53 16 8.03 16 3.61 12.406 0 7.984 0 3.563 0 0 3.61 0 8.031a7.88 7.88 0 0 0 5.172 7.422c.422.156.828-.125.828-.547v-1.25c-.219.094-.5.156-.75.156-1.031 0-1.64-.562-2.078-1.609-.172-.422-.36-.672-.719-.719-.187-.015-.25-.093-.25-.187 0-.188.313-.328.625-.328.453 0 .844.281 1.25.86.313.452.64.655 1.031.655s.641-.14 1-.5c.266-.265.47-.5.657-.656"/></svg>`;
  // The lucide icons of the Hushpen sidebar and Home rows
  const LU = {
    house: '<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    clock: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
    files: '<path d="M20 7h-3a2 2 0 0 1-2-2V2"/><path d="M9 18a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h7l4 4v10a2 2 0 0 1-2 2Z"/><path d="M3 7.6v12.8A1.6 1.6 0 0 0 4.6 22h9.8"/>',
    book: '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20"/><path d="m8 13 4-7 4 7"/><path d="M9.1 11h5.7"/>',
    cpu: '<rect width="16" height="16" x="4" y="4" rx="2"/><rect width="6" height="6" x="9" y="9" rx="1"/><path d="M15 2v2M15 20v2M2 15h2M2 9h2M20 15h2M20 9h2M9 2v2M9 20v2"/>',
    gear: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
    info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>',
    lines: '<path d="M2 10v3M6 6v11M10 3v18M14 8v7M18 5v13M22 10v3"/>',
    globe: '<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20"/>',
    mic: '<path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2M12 19v3"/>',
    chev: '<path d="m6 9 6 6 6-6"/>',
  };
  // The 12-tooth gear of the Yaru executable icon, with its round hole
  const gear = (c, ro, rb, rh, n) => {
    const P = (r, a) => `${(c + r * Math.sin(a)).toFixed(2)} ${(c - r * Math.cos(a)).toFixed(2)}`;
    const st = 2 * Math.PI / n, wb = st * .25, wt = st * .17;
    let d = '';
    for (let i = 0; i < n; i++) {
      const a = i * st;
      d += `${i ? 'L' : 'M'}${P(rb, a - wb)}L${P(ro, a - wt)}L${P(ro, a + wt)}L${P(rb, a + wb)}A${rb} ${rb} 0 0 1 ${P(rb, a + st - wb)}`;
    }
    return `${d}ZM${P(rh, 0)}A${rh} ${rh} 0 1 0 ${P(rh, Math.PI)}A${rh} ${rh} 0 1 0 ${P(rh, 0)}Z`;
  };
  // Yaru file icons (64 pt): the executable an AppImage shows, the folder, and the spreadsheet
  const FI = {
    exe: `<rect x="5" y="6.5" width="54" height="54" rx="5.5" fill="#000" opacity=".14"/><rect x="5" y="5" width="54" height="54" rx="5.5" fill="url(#lnx-exe)"/><rect x="5.5" y="5.5" width="53" height="53" rx="5" fill="none" stroke="#fff" stroke-opacity=".2"/><path d="${gear(32, 16.4, 13.2, 9, 12)}" fill="#F5F5F3" fill-rule="evenodd"/>`,
    folder: '<path d="M8 8h15.6a3 3 0 0 1 2.3 1.1L28.4 12H56a3.5 3.5 0 0 1 3.5 3.5V52a3.5 3.5 0 0 1-3.5 3.5H8A3.5 3.5 0 0 1 4.5 52V11.5A3.5 3.5 0 0 1 8 8z" fill="#A9330F"/><path d="M8 19.6h14.2a3 3 0 0 0 2.2-1l1.6-1.8a3 3 0 0 1 2.2-1H56a3.5 3.5 0 0 1 3.5 3.5V54a3.5 3.5 0 0 1-3.5 3.5H8A3.5 3.5 0 0 1 4.5 54V23.1a3.5 3.5 0 0 1 3.5-3.5z" fill="url(#lnx-fold)"/>',
    ods: '<path d="M15 5.5h23.5L51 18v38.5a2 2 0 0 1-2 2H15a2 2 0 0 1-2-2v-49a2 2 0 0 1 2-2z" fill="#FBFBFB" stroke="#000" stroke-opacity=".14"/><path d="M38.5 5.5V16a2 2 0 0 0 2 2H51" fill="#E4E4E4" stroke="#000" stroke-opacity=".14"/><rect x="19" y="27" width="26" height="23" rx="1.6" fill="#3AA757"/><path d="M19 34.6h26M19 42.2h26M27.7 27v23M36.4 27v23" stroke="#fff" stroke-width="1.5"/>',
  };
  // Dock icons (48 pt): real Firefox, Files and Text Editor icons, then Hushpen's own icon and Show Apps.
  const APP = {
    ff: '<img src="assets/linux/firefox.svg" alt="">',
    fil: '<img src="assets/linux/org.gnome.Nautilus.svg" alt="">',
    ed: '<img src="assets/linux/org.gnome.TextEditor.svg" alt="">',
    hp: '<img src="assets/icon-256.webp" alt="">',
    all: '<svg viewBox="0 0 48 48" aria-hidden="true">' + [14, 24, 34].map(y => [14, 24, 34].map(x => `<circle cx="${x}" cy="${y}" r="2.6" fill="#fff"/>`).join('')).join('') + '</svg>',
  };
  const DEFS = '<svg class="ln-defs" width="0" height="0" aria-hidden="true"><linearGradient id="lnx-exe" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#B4B4B4"/><stop offset="1" stop-color="#868686"/></linearGradient><linearGradient id="lnx-fold" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#E8693A"/><stop offset="1" stop-color="#DB582B"/></linearGradient><linearGradient id="lnx-fl" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#F4F4F4"/><stop offset="1" stop-color="#D5D5D5"/></linearGradient></svg>';

  const g = k => `<svg class="g" viewBox="0 0 16 16" aria-hidden="true">${G[k]}</svg>`;
  const lu = k => `<svg class="lu" viewBox="0 0 24 24" aria-hidden="true">${LU[k]}</svg>`;
  const fi = k => `<svg class="fi" viewBox="0 0 64 64" aria-hidden="true">${FI[k]}</svg>`;
  const ib = (k, c = '') => `<span class="ln-ib${c}">${g(k)}</span>`;
  const pos = ([x, y, w, h]) => `left:${x}px;top:${y}px;width:${w}px;height:${h}px`;
  // Ubuntu keeps minimize, maximize, and close at the right end of every header bar
  const WC = `<span class="ln-wc"><i>${g('min')}</i><i>${g('max')}</i><i>${g('x')}</i></span>`;
  // The Yaru pointer
  const CUR = '<svg class="ln-cur" viewBox="0 0 19 28" aria-hidden="true"><path d="M2 2v20.5l5-4.7 3.2 7.4 3.4-1.5-3.2-7.3H17z" fill="#111" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg>';

  // The GNOME 46 top bar: the workspace dots, the clock in the middle, then the tray and the system menu
  const top = () => `<div class="ln-top"><span class="ln-ws"><i></i><i></i></span><span class="ln-clk">Nov 2&nbsp; 9:41 AM</span><span class="ln-rt"><i class="ln-tray"></i><span class="ln-sys">${g('wifi')}${g('vol')}${g('bat')}</span></span></div>`;
  // The Ubuntu Dock in panel mode, with the orange dot of a running app
  const dock = () => `<div class="ln-dock">${['ff', 'fil', 'ed', 'hp'].map(k => `<span class="ln-di" data-a="${k}">${APP[k]}<i class="ln-run"></i></span>`).join('')}<span class="ln-sp"></span><span class="ln-di ln-all">${APP.all}</span></div>`;

  // GitHub's release page at the 600 px width of this Firefox window, logged
  // out and in the Light theme: github.com's markup and CSS values, at their real sizes
  const TABS = ['Code', 'Issues', 'Pull requests', 'Actions', 'Projects'];
  const AV = `<svg class="rl-av" viewBox="0 0 20 20" aria-hidden="true"><rect width="20" height="20" rx="3"/><path transform="translate(2.5 2.5) scale(.625)" d="${MKP}"/></svg>`;
  const asset = kind => `<li class="rl-ro${kind === 'linux-x64-appimage' ? ' gh-pick' : ''}" data-ln-asset="${kind}"><div class="rl-r1">${oc('pkg', 'rl-ic')}<a href="https://github.com/surajmandalcell/hushpen/releases/latest"><b>Latest version</b></a></div>
    <div class="rl-r2"><div class="rl-sh"><span class="rl-tr"><span></span></span><i class="rl-cp">${oc('copy')}</i></div><span class="rl-sz"></span><span class="rl-dt"></span></div></li>`;
  const page = () => `<div class="gh"><div class="rl-pg">
    <header class="rl-hd"><span class="rl-lg">${GH24}</span><span class="ln-sp"></span><span class="rl-bt">${oc('person')}</span><span class="rl-bt rl-tg">${oc('bars')}</span></header>
    <div class="rl-gn"><div class="rl-cx"><span class="rl-cr">surajmandalcell</span><i>/</i><span class="rl-cr rl-cl">hushpen</span></div>
      <nav class="rl-ln"><ul>${TABS.map((t, i) => `<li><span${i ? '' : ' class="on"'}>${t}</span></li>`).join('')}</ul><div class="rl-mr"><u></u><span>More${oc('tri')}</span></div></nav></div>
    <div class="rl-ct"><ol class="rl-bc"><li><a>Releases</a></li><li data-ln-tag>Latest version</li></ol>
      <div class="rl-bx"><div class="rl-bb">
        <div class="rl-ti"><div class="rl-h" data-ln-title>Latest version</div><span class="rl-lb">Latest</span></div>
        <div class="rl-cm"><span>Compare${oc('tri')}</span></div>
        <div class="rl-mt"><div>${AV} <b>github-actions</b> <span data-ln-published></span></div><div>${oc('tag')} <span data-ln-tag>Latest version</span></div></div>
        <div class="rl-md" data-ln-highlight hidden></div></div>
      <div class="rl-bf"><div class="rl-sm"><i></i><b>Assets</b><em data-ln-count></em></div>
        <div class="rl-ls"><ul>${ASSETS.map(asset).join('')}</ul></div></div></div></div>
  </div></div>`;

  const ff = r => `<div class="ln-win ln-ff" style="${pos(r)}">
    <div class="ff-tabs">${ib('fxv')}<span class="ff-tab">${GHM('ff-fav')}<span data-ln-tab>Latest version · surajmandalcell/hushpen · GitHub</span>${g('x')}</span>${ib('plus')}<span class="ln-sp"></span>${ib('chev', ' ff-lt')}${WC}</div>
    <div class="ff-nav">${ib('back')}${ib('fwd', ' ln-off')}${ib('reload')}<span class="ff-url">${g('lock')}<span data-ln-url><b>github.com</b>/surajmandalcell/hushpen/releases/latest</span></span><span class="ln-ib ff-dlb">${g('dl')}<i class="ff-dlp"><b></b></i></span>${ib('acct', ' ff-ac')}${ib('menu')}</div>
    ${page()}
    <div class="ff-pop"><div class="ff-it">${fi('exe')}<div class="ff-tx"><b data-ln-file>${FILE}</b><i class="ff-bar"><b></b></i><span class="ff-st">Starting…</span></div></div><div class="ff-pf">Show all downloads</div></div>
  </div>`;

  const SIDE = [['recent', 'Recent'], ['star', 'Starred'], ['home', 'Home'], ['desk', 'Desktop'], ['doc', 'Documents'], ['down', 'Downloads'], ['music', 'Music'], ['pic', 'Pictures'], ['video', 'Videos'], ['trash', 'Trash']];
  const files = r => `<div class="ln-win ln-fil" style="${pos(r)}">
    <div class="fl-sd"><div class="ln-hb fl-sh"><span class="ln-sp"></span>${ib('menu')}</div>${SIDE.map(([k, n]) => `<span class="fl-r${n === 'Downloads' ? ' on' : ''}">${g(k)}${n}</span>`).join('')}</div>
    <div class="fl-mn"><div class="ln-hb">${ib('back')}${ib('fwd', ' ln-off')}<span class="fl-pt">${g('home')}<span>Home</span><i>/</i><b>Downloads</b>${g('dots')}</span>${ib('search', ' fl-x')}${ib('grid', ' fl-x')}${WC}</div>
      <div class="fl-gr"><span class="fl-it">${fi('folder')}<span>trip-photos</span></span><span class="fl-it">${fi('ods')}<span>budget-2026.ods</span></span><span class="fl-it fl-app">${fi('exe')}<span data-ln-file>${FILE}</span></span></div>
      <div class="fl-st">AppImage selected</div>
    </div>
  </div>`;

  const NAV = [['house', 'Home'], ['clock', 'History'], ['files', 'Manual'], ['book', 'Dictionary'], ['cpu', 'Models'], ['gear', 'Settings'], ['info', 'About']];
  // Hushpen's own Linux window at its real 940 by 600, scaled: Light tokens, close and minimize only
  const hp = ([x, y], z) => `<div class="ln-win ln-hp" style="${pos([x, y, 940 * z, 600 * z])}"><div class="ln-hpz" style="zoom:${z}">
    <div class="ln-hs"><span class="ln-tl"><i></i><i></i></span><b class="ln-hn">Hushpen</b><div class="ln-nav">${NAV.map(([k, n], i) => `<span${i ? '' : ' class="on"'}>${lu(k)}${n}</span>`).join('')}</div></div>
    <div class="ln-hm"><b class="ln-ht">Home</b>
      <div class="ln-hg">
        <div class="ln-hr">${lu('lines')}<span>Dictation</span><em class="ln-hst">Ready</em><i class="ln-hbt">Record</i></div>
        <div class="ln-htr">Your words appear here and are copied to the clipboard.</div>
        <div class="ln-hr">${lu('globe')}<span>Language<small>Detects the language you speak</small></span><i class="ln-hpop">Auto${lu('chev')}</i></div>
      </div>
      <div class="ln-hg"><div class="ln-hr">${lu('mic')}<span>Microphone<small>Dictation listens on this microphone</small></span><i class="ln-hpop">Default (Built-in Audio)${lu('chev')}</i><i class="ln-hbt">Test microphone</i></div></div>
    </div>
  </div></div>`;

  const edit = r => `<div class="ln-win ln-ed" style="${pos(r)}"><div class="ln-hb"><span class="ln-btn">Open${g('chev')}</span>${ib('plus')}<span class="ln-ti"><b>Untitled Document 1</b><small>Draft</small></span><span class="ln-sp"></span>${ib('menu')}${WC}</div><div class="ln-doc"><span class="ln-tx"></span><i class="ln-caret"></i></div></div>`;
  // A keystroke overlay, as in a screen recording, and the flow bar under it
  const KC = '<div class="ln-kc" aria-hidden="true"><kbd>Right Alt</kbd><span>Double tap Right Alt</span></div>';

  // The logical desktop, its three windows, and where the pointer rests
  const L = { w: 880, h: 520, fb: 1.25, z: .5, ff: [84, 38, 600, 420], fil: [262, 118, 560, 342], hp: [350, 84], ed: [136, 52, 600, 332], rest: [600, 440] };
  const scene = () => `<div class="ln-dk" style="width:${L.w}px;height:${L.h}px">${top()}${dock()}${ff(L.ff)}${files(L.fil)}${hp(L.hp, L.z)}${edit(L.ed)}${KC}<div class="fbh ln-fb" style="--s:${L.fb}"><div data-fb data-hidden></div></div>${CUR}</div>`;

  const ARIA = 'An Ubuntu desktop: Firefox downloads the latest Hushpen AppImage, a double-click in Files opens Hushpen, and Hushpen types the first dictation into Text Editor.';
  host.insertAdjacentHTML('afterbegin', DEFS + `<div class="ln-st ln-wide" role="img" aria-label="${ARIA}"></div><ol class="ln-steps">${STEPS.map(([t, d], i) => `<li><b class="ln-n">${i + 1}</b><div><b class="ln-tt">${t}</b><span class="ln-ds">${d}</span></div></li>`).join('')}</ol>`);

  // A point or box in the desktop's own coordinates. It holds while the camera moves, since the
  // camera scales and moves the whole desktop.
  const lr = (S, n) => {
    const d = S.el.getBoundingClientRect(), k = d.width / S.L.w || 1, a = n.getBoundingClientRect();
    return [(a.left - d.left) / k, (a.top - d.top) / k, a.width / k, a.height / k];
  };
  // The camera: the whole desktop fills the stage until there is an action to follow. Then it closes in
  // on the action as far as the box allows, and on a phone it zooms to a readable width.
  const frame = (S, ms) => {
    const sw = S.st.clientWidth, sh = S.st.clientHeight;
    if (!sw || !sh) return;
    const { w: W, h: H } = S.L, f = S.focus, narrow = sw < 560, base = Math.max(sw / W, sh / H);
    let s = base, cx = W / 2, cy = sh / base / 2;
    if (f) {
      const [x, y, w, h] = f.r;
      s = narrow ? Math.max(base, sw / f.fw) : Math.max(base, Math.min(sw / (w + 48), sh / (h + 48), sw / 600));
      cx = x + w / 2; cy = y + h / 2;
    }
    const tx = Math.min(0, Math.max(sw - W * s, sw / 2 - cx * s)), ty = Math.min(0, Math.max(sh - H * s, sh / 2 - cy * s));
    S.el.style.transition = ms && !S.instant ? `transform ${ms}ms ${EASE}` : 'none';
    S.el.style.transform = `translate(${tx.toFixed(1)}px, ${ty.toFixed(1)}px) scale(${s.toFixed(4)})`;
  };
  const cam = (S, t, fw = 440, ms = 900) => {
    S.focus = t ? { r: Array.isArray(t) ? t : lr(S, t), fw } : null;
    frame(S, ms);
  };

  const live = async P => { while (!P.vis) await sleep(200); };
  // Waits, then holds while the layout is off screen or hidden
  const beat = async (S, ms) => {
    if (S.instant) return;
    await sleep(ms);
    await live(S.P);
  };
  // The pointer only glides. A click shows on what it presses, so the pointer never jumps.
  const move = async (S, t, ms) => {
    if (!S.instant) await live(S.P);
    const [x, y, w = 0, h = 0] = Array.isArray(t) ? t : lr(S, t);
    S.cur.style.transition = S.instant ? 'none' : `transform ${ms}ms ${HAND}`;
    S.cur.style.transform = `translate(${(x + w / 2 - 2).toFixed(1)}px, ${(y + h / 2 - 2).toFixed(1)}px)`;
    await beat(S, ms);
  };
  const click = async (S, t) => {
    t.classList.add('is-press');
    await beat(S, 110);
    t.classList.remove('is-press');
  };
  const $ = (S, sel) => S.el.querySelector(sel);
  const dockI = (S, k) => $(S, `.ln-di[data-a=${k}]`);
  const run = (S, k) => dockI(S, k).classList.add('is-in', 'is-run');
  const front = (S, w) => {
    S.el.querySelectorAll('.ln-win.is-fr').forEach(n => n.classList.remove('is-fr'));
    w.classList.add('is-open', 'is-fr');
    w.style.zIndex = ++S.z;
  };
  // The Windows scene (wnx.js) runs on the same camera and pointer
  window.LnScene = { lr, frame, cam, beat, move, click, front };

  const ST = ['is-open', 'is-fr', 'is-run', 'is-in', 'is-on', 'is-done', 'is-hov', 'is-sel', 'is-st', 'is-down', 'is-press'];
  // Back to the first frame with every transition off
  const reset = S => {
    const el = S.el;
    el.classList.add('ln-0');
    el.querySelectorAll(ST.map(c => '.' + c).join()).forEach(n => n.classList.remove(...ST));
    el.querySelectorAll('.ln-tx').forEach(n => { n.textContent = ''; });
    el.querySelectorAll('.ff-bar > b, .ff-dlp > b').forEach(b => { b.style.transition = ''; b.style.transform = ''; });
    el.querySelectorAll('.ff-st').forEach(n => { n.textContent = 'Starting…'; });
    el.querySelectorAll('.rl-pg').forEach(n => { n.style.setProperty('--sy', 0); n.style.transition = ''; });
    S.fb?.hide();
    run(S, 'ff'); front(S, $(S, '.ln-ff'));
    S.cur.style.transition = 'none';
    S.cur.style.transform = `translate(${S.L.rest[0]}px, ${S.L.rest[1]}px)`;
    S.focus = null;
    void el.offsetWidth;
    el.classList.remove('ln-0');
  };

  async function download(S) {
    const w = $(S, '.ln-ff'), row = w.querySelector('.gh-pick'), pop = w.querySelector('.ff-pop'), btn = w.querySelector('.ff-dlb'), st = pop.querySelector('.ff-st');
    const fills = [...w.querySelectorAll('.ff-bar > b, .ff-dlp > b')];
    // The page scrolls down to the Assets list as the camera closes in; the camera and the pointer aim at where
    // the row ends up, since the desktop's own units are page pixels
    const pg = w.querySelector('.rl-pg'), view = lr(S, w.querySelector('.gh')), r0 = lr(S, row), a0 = lr(S, row.querySelector('a'));
    const sy = Math.max(0, Math.round(r0[1] - view[1] - 100)), r = [r0[0], r0[1] - sy, r0[2], r0[3]], a = [a0[0], a0[1] - sy, a0[2], a0[3]];
    pg.style.transition = S.instant ? 'none' : '';
    pg.style.setProperty('--sy', sy);
    // On a phone the row is wider than the view, so the camera keeps the icon, the name, and the checksum
    cam(S, S.st.clientWidth < 560 ? [r[0], r[1], 330, r[3]] : r, 440);
    await move(S, a, 820);
    row.classList.add('is-hov');
    await beat(S, 200);
    await click(S, row);
    btn.classList.add('is-on');
    await beat(S, 160);
    pop.classList.add('is-open');
    cam(S, [...lr(S, pop).slice(0, 2), 300, 120], 440);
    await beat(S, 240);
    fills.forEach(b => { b.style.transition = S.instant ? 'none' : `transform ${PROG.length * 300}ms linear`; b.style.transform = 'scaleX(1)'; });
    for (const p of PROG) { st.textContent = bytes ? `${size(bytes * p)} of ${size(bytes)}` : 'Downloading'; await beat(S, 300); }
    st.textContent = bytes ? `Completed · ${size(bytes)}` : 'Completed';
    row.classList.remove('is-hov');
    pop.classList.add('is-done'); btn.classList.add('is-done');
    $(S, '.fl-app').classList.add('is-in');
  }

  async function launch(S) {
    const f = $(S, '.ln-fil'), it = f.querySelector('.fl-app'), win = $(S, '.ln-hp'), [hx, hy] = S.L.hp;
    const di = dockI(S, 'fil');
    cam(S, di, 440);
    await move(S, di, 800);
    await click(S, di);
    // A click outside closes the downloads panel, and Files opens on Downloads
    $(S, '.ff-pop').classList.remove('is-open');
    run(S, 'fil');
    await beat(S, 220);
    front(S, f);
    await beat(S, 360);
    cam(S, it, 440);
    await move(S, it.querySelector('svg'), 760);
    await beat(S, 140);
    it.classList.add('is-sel'); f.classList.add('is-st');
    await click(S, it);
    await beat(S, 60);
    await click(S, it);
    await beat(S, 380);
    front(S, win);
    run(S, 'hp');
    await beat(S, 380);
    $(S, '.ln-tray').classList.add('is-on');
    cam(S, S.st.clientWidth >= 560 ? [hx, hy, 470, 300] : [hx - 6, hy - 6, 250, 170], 250);
    await move(S, [hx + 300, hy + 250], 700);
    await beat(S, 700);
  }

  async function talk(S) {
    const kc = $(S, '.ln-kc'), tx = $(S, '.ln-tx'), ed = $(S, '.ln-ed'), [ex, ey, ew, eh] = S.L.ed;
    const di = dockI(S, 'ed');
    cam(S, di, 440);
    await move(S, di, 760);
    await click(S, di);
    await beat(S, 220);
    front(S, ed);
    // The keys and the flow bar sit on the desktop's center line, so the camera stays centered on that line
    const mid = S.L.w / 2, half = Math.max(mid - ex, ex + ew - mid);
    cam(S, [mid - half, ey, half * 2, S.L.h - ey], 440);
    await beat(S, 260);
    await move(S, [ex + ew - 70, ey + eh - 50], 640);
    kc.classList.add('is-on');
    await beat(S, 420);
    // A double tap on Right Alt starts hands-free, and a second one stops it
    const tap = async () => { kc.classList.add('is-down'); await beat(S, 100); kc.classList.remove('is-down'); };
    const press = async () => { await tap(); await beat(S, 90); await tap(); };
    await press();
    S.fb.set('listening').show();
    await beat(S, 2000);
    await press();
    S.fb.set('transcribing');
    await beat(S, 850);
    S.fb.set('undo');
    tx.textContent = SAID;
    kc.classList.remove('is-on');
    await beat(S, 1900);
    if (!S.instant) S.fb.hide();
    await beat(S, 400);
  }
  const ACTS = [download, launch, talk];

  const mk = (st, P) => {
    st.innerHTML = scene();
    const el = st.firstElementChild;
    const S = { P, st, el, L, z: 10, focus: null, instant: calm, cur: el.querySelector('.ln-cur') };
    S.fb = FlowBar.create(el.querySelector('[data-fb]'), { scale: L.fb, seed: 31, hidden: true });
    new ResizeObserver(() => frame(S, 0)).observe(st);
    return S;
  };
  const D = [3800, 5600, 8300];

  // The step list under the scene marks the step that plays
  const items = [...host.querySelectorAll('.ln-steps > li')];
  const set = i => items.forEach((c, j) => c.classList.toggle('is-on', j === i));
  const P = { vis: false };
  const S = mk(host.querySelector('.ln-st'), P);
  window.HushpenReleases.subscribe(({ stable }) => {
    const app = stable.assets['linux-x64-appimage'];
    if (app) {
      FILE = app.name; bytes = app.size;
      host.querySelectorAll('[data-ln-file]').forEach(el => { el.textContent = FILE; });
      host.querySelector('.fl-st').textContent = `${FILE} selected (${size(bytes)})`;
      items[0].querySelector('.ln-ds').textContent = `Get the AppImage. One file, ${size(bytes)}.`;
    }
    host.querySelectorAll('[data-ln-tag]').forEach(el => { el.textContent = stable.tag; });
    host.querySelector('[data-ln-title]').textContent = `Hushpen ${stable.version}`;
    host.querySelector('[data-ln-tab]').textContent = `Release Hushpen ${stable.version} · surajmandalcell/hushpen · GitHub`;
    host.querySelector('[data-ln-url]').textContent = stable.notesUrl.replace('https://', '');
    host.querySelector('[data-ln-published]').textContent = `released ${stable.publishedAt.slice(0, 10)}`;
    const note = host.querySelector('[data-ln-highlight]');
    note.textContent = stable.highlight || ''; note.hidden = !stable.highlight;
    host.querySelector('[data-ln-count]').textContent = Object.keys(stable.assets).length;
    host.querySelectorAll('[data-ln-asset]').forEach(row => {
      const a = stable.assets[row.dataset.lnAsset];
      row.hidden = !a;
      if (!a) return;
      row.querySelector('a').href = a.url;
      row.querySelector('a b').textContent = a.name;
      row.querySelector('.rl-tr span').textContent = `sha256:${a.sha256}`;
      row.querySelector('.rl-sz').textContent = size(a.size);
      row.querySelector('.rl-dt').textContent = stable.publishedAt.slice(0, 10);
    });
  });
  new IntersectionObserver(([e]) => {
    P.vis = e.isIntersecting;
    host.classList.toggle('ln-live', P.vis);
  }, { threshold: 0.15 }).observe(host);
  if (calm) {
    reset(S);
    (async () => { for (const act of ACTS) await act(S); frame(S, 0); })();
    set(-1);
    return;
  }
  const T = { P, instant: false };
  (async () => {
    for (let n = 0; ; n++) {
      if (n) { host.classList.add('ln-fade'); await beat(T, 380); }
      reset(S); set(-1);
      frame(S, n ? 700 : 0);
      host.classList.remove('ln-fade');
      await beat(T, n ? 500 : 300);
      for (let i = 0; i < 3; i++) {
        set(i);
        const t0 = performance.now();
        await beat(T, 160);
        await ACTS[i](S);
        await beat(T, Math.max(250, D[i] - (performance.now() - t0)));
      }
      set(3);
      await beat(T, 1600);
    }
  })();
})();
