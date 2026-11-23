// Home page hero: "Follow the words". Microsoft Teams and Slack tiled on a Mac screen, a camera that
// pushes in on the flow bar and the message box while you talk, then the Hushpen Home counting the words. Each
// frame is a pure function of the loop time t (ms); only the flow bar eases on its own. It plays only while the
// box is on screen; Reduce Motion shows the whole desktop with Home. <div class="hs-st" data-wm> inside the
// hero .hv-wm. ?t=1234 freezes it at that time, for screenshots.
(() => {
  if (!document.querySelector('[data-wm]') || !window.FlowBar || !window.WM_ICONS) return;
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const cl = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const pr = (t, a, d) => cl((t - a) / d);
  const eio = x => x < .5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
  const eo = x => 1 - Math.pow(1 - x, 3);
  const EASE = { punch: x => x >= 1 ? 1 : 1 - Math.pow(2, -10 * x), io: eio, sine: x => (1 - Math.cos(Math.PI * x)) / 2 };
  const lerp = (a, b, k) => a + (b - a) * k;
  const fade = (t, a, b, d = 200) => Math.min(pr(t, a, d), 1 - pr(t, b, d)); // in at a, out at b
  const op = (el, v) => { el.style.opacity = v.toFixed(3); el.style.visibility = v > 0 ? 'visible' : 'hidden'; };
  const grouped = v => Math.round(v).toLocaleString('en-US');
  const drive = (b, list, t) => {
    let s = null; for (const [at, st] of list) if (t >= at) s = st;
    if (s === b.cur) return; b.cur = s;
    if (s) { b.bar.set(s); b.bar.show(); } else b.bar.hide();
  };

  // Teams uses Fluent UI System Icons (MIT); Hushpen uses its own Lucide icons (ISC); both from why-m-icons.js
  const IC = window.WM_ICONS;
  const F = (n, s = 20) => { const [vb, d] = IC.fl[n]; return `<svg width="${s}" height="${s}" viewBox="${vb}" fill="currentColor" aria-hidden="true"><path d="${d}"/></svg>`; };
  const LU = (n, s = 16) => `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${IC.lu[n]}</svg>`;
  // Slack has no open icon set, so its glyphs are drawn by hand on a 20 point grid
  const P = {
    chevL: 'M12 4l-6 6 6 6', chevR: 'M8 4l6 6-6 6', chevD: 'M6 8l4 4 4-4', search: 'M8.5 3.5a5 5 0 1 0 0 10 5 5 0 0 0 0-10zM12.2 12.2L16.5 16.5',
    bell: 'M10 3a5 5 0 0 0-5 5v3l-1.5 2.5h13L15 11V8a5 5 0 0 0-5-5zM8 16a2 2 0 0 0 4 0', chat: 'M4 4.5h12v9H8.5L5 16.5v-3H4z',
    dots: 'M5 10h.01M10 10h.01M15 10h.01', video: 'M3 6h10v8H3zM13 9l4-2.2v6.4L13 11', filter: 'M3.5 5.5h13M6 10h8M8.5 14.5h3',
    emoji: 'M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM7 11.5a3.6 3.6 0 0 0 6 0M7.5 8.2h.01M12.5 8.2h.01',
    plus: 'M10 4v12M4 10h12', send: 'M3.5 10L16.5 3.5 13 16.5l-3-5.5z', home: 'M3.5 9L10 3.5 16.5 9v7.5H12v-5H8v5H3.5z',
    mic: 'M10 3a2 2 0 0 0-2 2v5a2 2 0 0 0 4 0V5a2 2 0 0 0-2-2zM5.8 10a4.2 4.2 0 0 0 8.4 0M10 14.2V17', clock: 'M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM10 6.5V10l2.5 1.5',
    help: 'M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM8 8a2 2 0 1 1 2.8 1.8c-.5.3-.8.7-.8 1.2M10 13.6h.01', at: 'M13.2 10a3.2 3.2 0 1 1-1.1-2.4V11a1.6 1.6 0 0 0 3.2 0v-1a6.3 6.3 0 1 0-2.4 5',
    slash: 'M4.5 4.5h11v11h-11zM11.5 7l-3 6', head: 'M4 12v-1a6 6 0 0 1 12 0v1M4 12h2.5v4H4zM13.5 12H16v4h-2.5z',
    homeF: 'M10 3.2 3.6 8.7v7.8h12.8V8.7z', dms: 'M8.2 4.5a4.3 4.3 0 0 0-3.9 6.2l-.8 2.6 2.6-.8a4.3 4.3 0 1 0 2.1-8zM12.9 8.3a3.8 3.8 0 0 1 3.1 5.9l.7 2.3-2.3-.7a3.8 3.8 0 0 1-5-1.2',
    canvas: 'M6.5 3.5h7a2 2 0 0 1 2 2v5L11 15.5H6.5a2 2 0 0 1-2-2v-1M15.5 10.5H13a2 2 0 0 0-2 2v3M3 5.5h4M5 3.5v4',
    copilot: 'M7.5 4h5a2 2 0 0 1 1.9 1.4l1.9 6.3a2 2 0 0 1-1.9 2.6H9.6M12.5 16H7.5a2 2 0 0 1-1.9-1.4L3.7 8.3a2 2 0 0 1 1.9-2.6h4.8',
    sidebar: 'M3.5 4.5h13v11h-13zM8.5 4.5v11M5.3 7.2h1.4M5.3 9.6h1.4', arrL: 'M16 10H4.5M9 5.5 4.5 10 9 14.5', arrR: 'M4 10h11.5M11 5.5l4.5 4.5-4.5 4.5',
    bot: 'M10 2.8a7.2 7.2 0 1 0 0 14.4 7.2 7.2 0 0 0 0-14.4zM10 5.6l1.1 3.3 3.3 1.1-3.3 1.1-1.1 3.3-1.1-3.3L5.6 10l3.3-1.1z',
    homeO: 'M3.8 8.6 10 3.6l6.2 5v7.6H3.8zM10 9a1.4 1.4 0 1 0 0 2.8A1.4 1.4 0 0 0 10 9z',
    files: 'M7.5 3.5h6a2 2 0 0 1 2 2v6M5.5 6.5h6a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2h-6a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2z',
    agents: 'M6 3.6a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8zM6 11.6a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8zM14 11.6a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8zM14 3l.8 2.2 2.2.8-2.2.8L14 9l-.8-2.2L11 6l2.2-.8z',
    moon: 'M15.8 12.3A6.6 6.6 0 0 1 7.7 4.2a6.6 6.6 0 1 0 8.1 8.1z', compose: 'M9 4H5.5a2 2 0 0 0-2 2v8.5a2 2 0 0 0 2 2H14a2 2 0 0 0 2-2V11M14.2 3.3l2.5 2.5-6.7 6.7-3 .5.5-3z',
    findDm: 'M3.5 5.5h9M3.5 9.5h4M3.5 13.5h4M12.5 8.5a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM14.7 14.2l2 2', star: 'M10 3l2.1 4.4 4.8.6-3.5 3.3.9 4.7L10 13.7 5.7 16l.9-4.7L3.1 8l4.8-.6z',
    kebab: 'M10 4.5h.01M10 10h.01M10 15.5h.01', close: 'M5 5l10 10M15 5 5 15',
    link: 'M8.5 11.5l3-3M9 6.5l1.5-1.5a3 3 0 0 1 4.2 4.2l-1.5 1.5M11 13.5l-1.5 1.5a3 3 0 0 1-4.2-4.2l1.5-1.5',
    olist: 'M8.5 5.5h8M8.5 10h8M8.5 14.5h8M3.8 4.4l1-.6v3.4M3.6 12.6a1 1 0 0 1 1.8.5c0 .7-1.8 1.4-1.8 2.3h1.9', ulist: 'M8 5.5h8.5M8 10h8.5M8 14.5h8.5M4.2 5.5h.7M4.2 10h.7M4.2 14.5h.7',
    quote: 'M4.5 4v12M8.5 5.5h8M8.5 10h8M8.5 14.5h5', code: 'M7.5 6l-4 4 4 4M12.5 6l4 4-4 4M11 4.5l-2 11',
    cblock: 'M7 4.5H5A1.5 1.5 0 0 0 3.5 6v8A1.5 1.5 0 0 0 5 15.5h10a1.5 1.5 0 0 0 1.5-1.5V9.5M8.5 7.5 7 10l1.5 2.5M11.5 3.5l-1 4M14 3.5l-1 4', hr: 'M4 10h12M5.5 6.5V5h9v1.5M5.5 13.5V15h9v-1.5'
  };
  const I = (n, s = 20, w = 1.5) => `<svg width="${s}" height="${s}" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="${n === 'dots' || n === 'kebab' ? 2.4 : w}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${P[n]}"/></svg>`;
  // Made-up people with generated faces (thispersondoesnotexist.com), saved in assets/wm
  const AV = (who, s, dot = '') => `<span class="hs-av" style="width:${s}px;height:${s}px"><img src="assets/wm/${who}.webp" alt="">${dot}</span>`;
  const LOGO = (s, r) => `<b class="hs-logo" style="width:${s}px;height:${s}px;border-radius:${r}px"></b>`;
  // macOS window controls as vectors: 12 point circles 20 points apart, each with its darker 0.5 point rim
  const TLC = [['#FF5F57', '#E2463F'], ['#FEBC2E', '#E1A116'], ['#28C840', '#1AAB29']];
  const TLS = (n, d = 12, g = 20) => `<svg class="hs-tl" width="${g * (n - 1) + d}" height="${d}" viewBox="0 0 ${g * (n - 1) + d} ${d}" aria-hidden="true">${TLC.slice(0, n).map(([f, s], i) => `<circle cx="${d / 2 + g * i}" cy="${d / 2}" r="${d / 2 - .25}" fill="${f}" stroke="${s}" stroke-width=".5"/>`).join('')}</svg>`;
  const TL = TLS(3, 14, 23); // macOS 26 windows with a toolbar: 14 point circles, 23 points apart
  // Menu bar extras drawn after SF Symbols: battery.75percent, wifi, magnifyingglass, switch.2 (Control Center)
  const MS = (w, h, body) => `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" fill="currentColor" aria-hidden="true">${body}</svg>`;
  const HUSH = MS(22, 16, '<path d="M1.25 12.5Q2.38 10.75 3.5 12.5T5.75 12.5 8 12.5 10.25 12.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M21 .6C15.8 1.2 12.2 5 11.2 10l1.4-.8C15.2 8.6 18 6.2 18.9 4c-.8.2-1.6.2-2.2 0C18.2 3.4 20.1 2.2 21 .6z"/><path d="M12.6 9.2 10.5 12" fill="none" stroke="currentColor" stroke-linecap="round"/>');
  const BAT = `<span class="hs-bat">${MS(26, 13, '<rect x=".75" y=".75" width="21.5" height="11.5" rx="3.6" fill="none" stroke="currentColor" stroke-opacity=".4" stroke-width="1.1"/><rect x="2.5" y="2.5" width="14.2" height="8" rx="2"/><path d="M24 4.6v3.8" stroke="currentColor" stroke-opacity=".4" stroke-width="1.6" stroke-linecap="round"/>')}</span>`;
  const WIFI = MS(18, 14, '<path d="M9 13.4 6.9 11a3 3 0 0 1 4.2 0z"/><path d="M4.7 8.6a6.2 6.2 0 0 1 8.6 0l-1.5 1.7a4 4 0 0 0-5.6 0z"/><path d="M2.4 6.1a9.6 9.6 0 0 1 13.2 0l-1.5 1.7a7.3 7.3 0 0 0-10.2 0z"/><path d="M.1 3.6a13 13 0 0 1 17.8 0l-1.5 1.7a10.7 10.7 0 0 0-14.8 0z"/>');
  const SPOT = MS(15, 15, '<circle cx="6.2" cy="6.2" r="4.9" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M9.8 9.8 14 14" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"/>');
  const CC = MS(17, 15, '<rect x=".8" y=".8" width="15.4" height="5.6" rx="2.8" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="3.9" cy="3.6" r="1.8"/><path fill-rule="evenodd" d="M3.6 8.2h9.8a3.2 3.2 0 0 1 0 6.4H3.6a3.2 3.2 0 0 1 0-6.4zm9.5 1.4a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 0 0 0-3.6z"/>');
  const MB = `<div class="hs-mb"><svg class="hs-apl" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><use href="#i-apple"/></svg><b data-k="app">Microsoft Teams</b><span>File</span><span>Edit</span><span>View</span><span>Window</span><span>Help</span><span class="hs-rt">${HUSH}${BAT}${WIFI}${SPOT}${CC}<span class="hs-clk">Sun Nov 22&nbsp;&nbsp;9:41 AM</span></span></div>`;

  // Words that stream in one by one; a leading * marks a filler word that Hushpen cleans out
  const W = list => list.map(w => w[0] === '*' ? `<span class="hs-w f">${w.slice(1)}</span>` : `<span class="hs-w">${w}</span>`).join(' ');
  const TM_TEXT = 'Running ten minutes late, start without me.'.split(' ');
  const SK_TEXT = "I'll send you the draft before lunch.".split(' ');
  const TM_SAY = ['*uh', 'running', '*like', 'ten', 'minutes', 'late,', '*um,', 'start', 'without', 'me'];
  const SK_SAY = ['*um', '*so', "I'll", 'send', 'you', 'the', '*uh', 'draft,', '*like,', 'before', 'lunch'];

  // Teams and Slack are laid out at their real point sizes, measured on real windows,
  // in a window 1000 (Teams) or 1040 (Slack) points wide, then each window is scaled as a whole into its tile.
  const TAV = (who, s) => AV(who, s, '<i></i>');
  const INI = (t, s, bg, fg) => `<span class="hs-av tm-ini" style="width:${s}px;height:${s}px;background:${bg};color:${fg}">${t}<i></i></span>`;
  const MEET = `<span class="hs-av tm-ini tm-meet">${F('calendar_24_regular', 16)}<i></i></span>`;
  const RAIL = [['alert_24_regular', 'Activity'], ['call_24_regular', 'Calls'], ['chat_24_filled', 'Chat'], ['calendar_24_regular', 'Calendar'], ['people_team_24_regular', 'Teams'],
    ['cloud_24_regular', 'OneDrive'], ['copilot', 'Copilot'], ['heart_pulse_24_regular', 'Viva Insights'], ['book_open_24_regular', 'Viva Learning'], ['backpack_24_regular', 'Assignments'],
    ['location_24_regular', 'Places'], ['add_square_24_regular', 'Store']];
  const TROW = (av, n, d, p, on) => `<div class="tm-row${on ? ' on' : ''}">${av}<b>${n}</b><em>${d}</em><span>${p}</span></div>`;
  const TMSG = (me, time, lines, extra = '') => `<div class="tm-m${me ? ' me' : ''}">${me ? TAV('daniel', 32) : TAV('emma', 32)}<p class="tm-nm"><b>${me ? 'Daniel Brooks' : 'Emma Clarke'}</b>${time}</p>${lines.map(l => `<p>${l}</p>`).join('')}${extra}</div>`;
  const TEAMS = `<div class="hs-sc tm-sc">${TL}<div class="tm-top"><span class="tm-nav">${F('chevron_left_20_regular', 16)}${F('chevron_right_20_regular', 16)}</span><span class="tm-search">${F('search_20_regular', 16)}Press ⌘ G to go right to a chat or channel</span><span class="tm-more">${F('more_horizontal_20_regular', 16)}</span><span class="tm-me">${AV('daniel', 24)}</span></div>
    <div class="tm-body"><nav class="tm-rail">${RAIL.map(([ic, n]) => `<span${n === 'Chat' ? ' class="on"' : ''}>${ic === 'copilot' ? I(ic, 20, 1.3) : F(ic, 20)}<small>${n}</small></span>`).join('')}<span class="tm-rm">${F('more_horizontal_24_regular', 20)}</span><span class="tm-help">${F('person_feedback_24_regular', 20)}</span></nav>
    <div class="tm-list"><div class="tm-lh"><b>Chat</b>${F('more_horizontal_20_regular', 18)}${F('search_20_regular', 18)}<span class="tm-cz">${F('compose_20_regular', 18)}${F('chevron_down_20_regular', 14)}</span></div>
      <div class="tm-chips"><span>Unread</span><span>Chats</span><span>Meeting chats</span></div>
      <div class="tm-li tm-cop">${I('copilot', 18, 1.3)}Copilot</div><div class="tm-sec tm-qv">${F('chevron_down_20_regular', 12)}Quick views</div><div class="tm-li tm-dr">${F('edit_20_regular', 18)}Drafts</div>
      <div class="tm-sec tm-fav">${F('chevron_down_20_regular', 12)}Favourites</div>
      ${TROW(TAV('daniel', 32), 'Daniel Brooks (You)', '2025-11-02', 'You: Slides for the Friday demo')}
      ${TROW(MEET, 'Launch weekly sync', '11-20', 'Emma: Notes are in the channel')}
      ${TROW(INI('HW', 32, '#CDEDE9', '#0E5A54'), 'Harry Walsh, +2', '11-18', 'Harry: The build is green again')}
      <div class="tm-sec">${F('chevron_down_20_regular', 12)}Chats</div>
      ${TROW(INI('JP', 32, '#E3E1F7', '#444791'), 'James Porter', '11-21', 'Sent a file')}
      ${TROW(TAV('emma', 32), 'Emma Clarke', '09:38', 'I\'ll share the roadmap deck when we start.', true)}
      ${TROW(TAV('olivia', 32), 'Olivia Hughes', '11-19', 'You: Thanks, that works for me')}
      ${TROW(INI('LT', 32, '#FBEBC4', '#8A5A00'), 'Lucy Turner', '11-12', 'Lucy: Can you check the copy?')}
      ${TROW(MEET, 'Design review', '11-07', 'Lucy: See you at 3')}
      ${TROW(TAV('harry', 32), 'Harry Walsh', '10-31', 'You: Pushed the fix, try again')}</div>
    <div class="tm-conv"><div class="tm-ch">${TAV('emma', 30)}<b>Emma Clarke</b><span class="tm-tab on">Chat</span><span class="tm-tab">Shared</span><span class="tm-tab">Notes</span><span class="tm-tab">Recap</span>${F('add_square_20_regular', 16)}
        <span class="tm-ci">${F('call_20_regular', 18)}${F('chevron_down_20_regular', 12)}${F('people_settings_20_regular', 18)}<i></i>${F('search_20_regular', 18)}${F('window_new_20_regular', 18)}${F('more_horizontal_20_regular', 18)}</span></div>
      <div class="tm-msgs">
        ${TMSG(0, '8:47 am', ['Quick one before the sync:', 'can you send me the link to the new site?'])}
        ${TMSG(1, '8:50 am', ['<u>https://samhq.app/hushpen</u>'], `<div class="tm-card"><i>${F('window_new_20_regular', 40)}</i><div><b>Hushpen - Dictation that cleans up as you talk</b><p>Talk in any app and Hushpen types clean text. Works offline on your Mac, no account needed.</p><small>samhq.app</small></div></div>`)}
        ${TMSG(0, '8:52 am', ['Love it. The new flow bar looks great.'])}
        ${TMSG(1, '9:04 am', ['The build went out last night, no crash reports so far.'])}
        ${TMSG(0, '9:05 am', ['Great. Can you add the release notes to the launch doc?'])}
        ${TMSG(1, '9:12 am', ['Done, they are under "What\'s new".', 'I also fixed the two typos you found.'])}
        ${TMSG(0, '9:20 am', ['Thanks! Harry wants a quick look at the onboarding flow too.'])}
        ${TMSG(1, '9:21 am', ['Sure, I\'ll walk him through it after the sync.'])}
        ${TMSG(0, '9:32 am', ['Are we still on for the 10:00 sync?'])}
        ${TMSG(1, '9:33 am', ['Yes, see you there.'])}
        ${TMSG(0, '9:38 am', ['I\'ll share the roadmap deck when we start.'])}
        ${TMSG(1, '9:41 am', [TM_TEXT.join(' ')]).replace('tm-m me', 'tm-m me hs-new" data-k="tmNew')}</div>
      <div class="tm-cmp" data-k="tmCmp"><span class="hs-field"><i class="hs-caret"></i><span class="hs-ph">Type a message</span><span class="hs-txt">${W(TM_TEXT)}</span></span><span class="tm-ic">${F('text_edit_style_20_regular', 18)}${F('emoji_20_regular', 18)}${F('attach_20_regular', 18)}${F('chat_bubbles_question_20_regular', 18)}${F('add_20_regular', 18)}</span><i class="tm-div"></i><span class="tm-send">${F('send_20_regular', 18)}</span></div></div></div></div>`;

  const SAV = (who, s, dot = false) => AV(who, s, dot ? '<i></i>' : '');
  const SROW = (who, n, d, p, on) => `<div class="sk-dm${on ? ' on' : ''}">${SAV(who, 36, true)}<b>${n}</b><em>${d}</em><p>${p}</p></div>`;
  const SRAIL = [['bot', 'Slackbot'], ['homeO', 'Home'], ['dms', 'DMs'], ['bell', 'Activity'], ['files', 'Files'], ['agents', 'Agents<br>&amp; tools']];
  const FILLED = s => s.replace('fill="none" stroke="currentColor"', 'fill="currentColor" stroke="currentColor"');
  const SLACK = `<div class="hs-sc sk-sc">${TL}<div class="sk-top"><span class="sk-sb">${I('sidebar', 20, 1.5)}</span><span class="sk-nav">${I('arrL', 20, 1.6)}${I('arrR', 20, 1.6)}${I('clock', 20, 1.6)}</span>
      <span class="sk-search">${I('search', 16, 1.7)}Search SamHQ</span><span class="sk-app"></span><span class="sk-help">${I('help', 20, 1.5)}</span></div>
    <nav class="sk-rail"><span class="sk-ws">${LOGO(36, 8)}</span>${SRAIL.map(([ic, n]) => `<span${n === 'DMs' ? ' class="on"' : ''}><i>${n === 'DMs' ? FILLED(I(ic, 22, 1)) : I(ic, 22, 1.6)}</i>${n}</span>`).join('')}
      <b class="sk-rb first">${I('plus', 22, 1.5)}</b><b class="sk-rb">${I('moon', 20, 1.5)}</b><span class="sk-me">${SAV('daniel', 36, true)}</span></nav>
    <div class="sk-pane"><div class="sk-side"><div class="sk-sh"><b>Direct messages</b>${I('chevD', 14, 1.8)}<span>Unread messages</span><i class="sk-tg"></i>${I('compose', 20, 1.5)}</div>
      <div class="sk-find">${I('findDm', 16, 1.4)}Find a DM...</div>
      ${SROW('olivia', 'Olivia Hughes', '11:26 AM', 'Final numbers are in the sheet. Can you get me the draft today?', true)}
      ${SROW('james', 'James Porter', 'Yesterday', 'You: Sounds good, thanks!')}
      ${SROW('emma', 'Emma Clarke', '20 November', 'The new site copy is ready for a look.')}
      ${SROW('harry', 'Harry Walsh', '18 November', 'You: I pushed the fix, can you try again?')}
      ${SROW('lucy', 'Lucy Turner', '12 November', 'Lunch on Friday? There is a new place near the office that does great ramen.')}
      ${SROW('daniel', 'Daniel Brooks <span>(you)</span>', '2 November', 'Notes for the Friday demo')}</div>
    <div class="sk-main"><div class="sk-ch">${I('star', 22, 1.5)}${SAV('olivia', 24)}<b>Olivia Hughes</b><span class="sk-ch-r">${I('head', 22, 1.6)}${I('chevD', 13, 1.9)}${I('bell', 22, 1.6)}${I('search', 22, 1.7)}${I('kebab', 20)}${I('close', 20, 1.6)}</span></div>
      <div class="sk-tabs"><span class="on"><svg width="16" height="16" viewBox="0 0 20 20" aria-hidden="true"><path d="M10 3a7 7 0 0 0-6 10.6L3 17l3.5-1A7 7 0 1 0 10 3z" fill="currentColor"/></svg>Messages</span><span>${I('canvas', 16, 1.4)}Add canvas</span><span>${I('plus', 16, 1.4)}</span></div>
      <div class="sk-msgs"><div class="sk-intro">${SAV('olivia', 102)}<b>Olivia Hughes</b></div><p class="sk-about">This conversation is just between you and <u>@Olivia Hughes</u>. Take a look at their profile to learn more about them.</p><span class="sk-vp">View profile</span>
        <p class="sk-day"><span>Today</span></p>
        <div class="sk-m">${SAV('olivia', 36)}<div><p>Olivia Hughes<em>11:26 AM</em></p><p>Final numbers are in the sheet. Can you get me the draft today?</p></div></div>
        <div class="sk-m hs-new" data-k="skNew">${SAV('daniel', 36)}<div><p>Daniel Brooks<em>11:31 AM</em></p><p>${SK_TEXT.join(' ')}</p></div></div></div>
      <div class="sk-cmp" data-k="skCmp"><span class="hs-field"><i class="hs-caret"></i><span class="hs-ph">Message Olivia Hughes</span><span class="hs-txt">${W(SK_TEXT)}</span></span>
        <div class="sk-tb"><b>B</b><b class="it">I</b><b class="un">U</b><b class="st">S</b><b>¶${I('chevD', 13, 1.9)}</b><i></i>${I('link', 20, 1.5)}${I('olist', 20, 1.5)}${I('ulist', 20, 1.5)}<i></i>${I('quote', 20, 1.5)}${I('code', 20, 1.5)}${I('cblock', 20, 1.5)}${I('hr', 20, 1.5)}</div>
        <div class="sk-bot"><span class="sk-plus">${I('plus', 20, 1.5)}</span><span class="sk-aa">Aa</span>${I('emoji', 20, 1.5)}${I('at', 20, 1.5)}<i></i>${I('video', 20, 1.5)}${I('mic', 20, 1.5)}<i></i>${I('slash', 20, 1.5)}<span class="sk-send"><span><svg width="16" height="16" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true"><path d="M3 3.2 17.8 10 3 16.8l1.9-6.8z"/></svg></span><i></i><span>${I('chevD', 12, 1.8)}</span></span></div></div></div></div></div>`;

  // Hushpen Home, rebuilt in HTML from the app's pixel baseline (940 by 600 points) so it stays sharp when zoomed
  const HEAT = ['0000100100102200010212222222022222222', '0000000021002220222022220212222222222', '0001120110012002111002022220222222222',
    '0000110000000202002222222221222222221', '000010020220202020022200120222220222-', '000000010000010000001000200111222201-', '000000000000000200022201120022002012-'];
  const CARDS = [['text-align-justify', 'Words', '<b data-k="num">50,707</b>', '2,676 this week'], ['gauge', 'Speed', '<b>149</b><small>wpm</small>', '3.7x your typing'],
    ['flame', 'Streak', '<b>12</b><small>days</small>', 'Best: 19 days'], ['hourglass', 'Time saved', '<b>15.4</b><small>h</small>', 'vs typing at 40 wpm']];
  const RECENT = [['Running about ten minutes late for standup, start without me and I\'ll catch up on the notes after.', 'Today 11:32 · Slack · 7.3 s'],
    ['Hi team, attached is the updated launch checklist. Please flag anything that looks off by Friday.', 'Today 11:05 · Mail · 6.7 s'],
    ['Add a retry with exponential backoff to the upload function and log the final error if all three att...', 'Today 10:48 · Cursor · 8.5 s'],
    ['Groceries for the weekend: oat milk, sourdough, two avocados, coffee beans, and something f...', 'Today 10:21 · Notes · 6.0 s']];
  const NAV = [['house', 'Home'], ['clock', 'History'], ['book-a', 'Dictionary'], ['cpu', 'Models'], ['smartphone', 'Devices'], ['settings', 'Settings'], ['info', 'About']];
  const HOME = `${TLS(2)}<b class="hp-name">Hushpen</b>
    <nav class="hp-nav">${NAV.map(([i, n], j) => `<span style="top:${48 + 30 * j}px"${j ? '' : ' class="on"'}>${LU(i)}${n}</span>`).join('')}</nav>
    <b class="hp-h">Home</b>
    ${CARDS.map(([i, n, v, s], j) => `<div class="hp-card hp-st" style="left:${256 + 168 * j}px"><span class="hp-lb">${LU(i, 12)}${n}</span><p class="hp-v">${v}</p><p class="hp-s">${s}</p></div>`).join('')}
    <div class="hp-card hp-heat"><b>Words per day</b><em class="hp-r">Last 37 weeks</em><div class="hp-grid">${HEAT.join('').split('').map(c => `<i class="l${c === '-' ? 'x' : c}"></i>`).join('')}</div>
      <span class="hp-leg">Less<i></i><i></i><i></i>More</span><em class="hp-r hp-today">Today: 160 words</em></div>
    <p class="hp-rh">Recent<span>Show more</span></p>
    <div class="hp-card hp-rec">${RECENT.map(([a, b]) => `<p><b>${a}</b><span>${b}</span></p>`).join('')}</div>`;

  // A 1512 by 945 point screen with no notch (16:10); the two apps tiled with macOS margins under the 30 point menu bar
  const VW = 1512, VH = 945, TMW = [8, 38, 744, 899], SKW = [760, 38, 744, 899], MID = VH / 2;
  // L: the lead where the pointer clicks into the Teams box before you talk. Slack starts at SK, once the camera has
  // pulled out and the pointer has clicked into Slack's box; Home at HP.
  const L = 1000, SK = 5400, HP = SK + 4200, T = HP + 3000;
  const BAR = [[L + 300, 'listening'], [L + 1500, 'cleaning'], [L + 2100, 'undo'], [L + 2900, null], [SK + 300, 'listening'], [SK + 1500, 'cleaning'], [SK + 2100, 'undo'], [SK + 2900, null]];
  // The pointer, in screen points: [ms, x, y] stops; it glides between stops. Clicks press at the given ms.
  // Teams box text starts at 328,906; Slack's at 1020,877 (measured at full view). macOS hides it while you type.
  const PT = [[0, 560, 640], [350, 560, 640], [800, 430, 906], [L + 3300, 430, 906], [4950, 430, 906], [SK - 120, 1140, 877]];
  const CLICK = [820, SK - 100], TF = 850; // Teams takes focus at TF, Slack at SK (the click makes it the front app)
  const SHOWN = [[0, L + 2100], [L + 3300, SK + 2100]];
  const ARROW = '<svg class="a" width="19" height="27" viewBox="0 0 19 27" aria-hidden="true"><path d="M2 2v19.2l4.6-4.4 3 7 3.3-1.4-3-6.9h6.4z" fill="#000" stroke="#fff" stroke-width="1.4" stroke-linejoin="round"/></svg>';
  const IBEAM = '<svg class="i" width="11" height="20" viewBox="0 0 11 20" aria-hidden="true"><path d="M2.5 1.5c1.8 0 3 .5 3 1.8v13.4c0 1.3-1.2 1.8-3 1.8M8.5 1.5c-1.8 0-3 .5-3 1.8M8.5 18.5c-1.8 0-3-.5-3-1.8M3.5 10h4" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round"/><path d="M2.5 1.5c1.8 0 3 .5 3 1.8v13.4c0 1.3-1.2 1.8-3 1.8M8.5 1.5c-1.8 0-3 .5-3 1.8M8.5 18.5c-1.8 0-3-.5-3-1.8M3.5 10h4" fill="none" stroke="#000" stroke-width="1.1" stroke-linecap="round"/></svg>';

  // Camera moves: [start ms, length ms, center x, center y, scale, easing], in screen points. Between moves it holds.
  // "Follow the words": close on the flow bar while you talk (its top is at 885) and a whip to the Teams message box
  // as the text lands; then one pull out to the full screen, where Slack and Home play with no more zooms.
  // The bubble of what you say sits just above the flow bar.
  const CAM = { say: 'bubble', at: [[756, 875], [756, 875]], moves: [
    [L + 100, 450, 756, 870, 3.2, 'punch'], [L + 1950, 380, 430, 880, 3.2, 'io'], [L + 2900, 400, 440, 800, 2.4, 'punch'], [L + 3400, 500, 756, MID, 1, 'io']] };
  // Center and scale at t: each move starts from where the last one ended; scale moves on a log scale so a zoom
  // feels even, and the view is clamped so the screen edge never shows.
  const camAt = (moves, t) => {
    let c = [VW / 2, VH / 2, 1];
    for (const [a, d, x, y, s, e] of moves) {
      if (t < a) break;
      const k = EASE[e](pr(t, a, d));
      c = [lerp(c[0], x, k), lerp(c[1], y, k), Math.exp(lerp(Math.log(c[2]), Math.log(s), k))];
    }
    const [x, y, s] = c, hw = VW / 2 / s, hh = VH / 2 / s;
    return [cl(x, hw, VW - hw), cl(y, hh, VH - hh), s];
  };
  // Words appear one by one between a and b; the newest one is marked "now"
  const stream = (el, t, a, b) => {
    const ws = el._w || (el._w = [...el.querySelectorAll('.hs-w')]), n = ws.length;
    ws.forEach((w, i) => {
      const at = a + (b - a) * i / n, k = eo(pr(t, at, 140)), v = k.toFixed(2);
      if (w._v !== v) { w._v = v; w.style.opacity = v; w.style.visibility = k > 0 ? 'visible' : 'hidden'; w.style.transform = k < 1 ? `translateY(${((1 - k) * .3).toFixed(3)}em)` : ''; }
      w.classList.toggle('now', t >= at && t < (i + 1 < n ? at + (b - a) / n : b + 250));
    });
  };

  const scene = host => {
    const cam = CAM;
    const box = ([x, y, w, h]) => `left:${x}px;top:${y}px;width:${w}px;height:${h}px`;
    const say = (k, list) => `<p class="hs-say" data-k="${k}">${W(list)}</p>`;
    // The wallpaper (White blossoms, 2048 pixels) sits on its own layer, so the GPU scales the decoded picture itself
    host.innerHTML = `<div class="hs-world"><div class="hs-cam"><img class="hs-bg" src="assets/wm/blossoms.webp" alt="">${MB}<div class="hs-g">
      <div class="hs-win tm" data-k="tm" style="${box(TMW)}">${TEAMS}</div>
      <div class="hs-win sk" data-k="sk" style="${box(SKW)}">${SLACK}</div>
      <div class="hs-win hs-hp" data-k="hp">${HOME}</div><div class="hs-ptr" data-k="ptr">${ARROW}${IBEAM}</div></div>
      <div class="hs-fb"><div data-fb></div></div></div></div>
      <div class="hs-ov">${say('sayTm', TM_SAY)}${say('saySk', SK_SAY)}</div>`;
    const E = k => host.querySelector(`[data-k="${k}"]`);
    const w = host.firstChild, layer = w.firstChild, g = w.querySelector('.hs-g'), tm = E('tm'), sk = E('sk'), hp = E('hp'), num = E('num'), app = E('app');
    const ptr = E('ptr'), tmNew = E('tmNew'), skNew = E('skNew'), tmCmp = E('tmCmp'), skCmp = E('skCmp'), sayTm = E('sayTm'), saySk = E('saySk');
    // The canvas is drawn at 2.2 times the screen size, enough for the deepest camera zoom on a 2x display
    const fb = { bar: FlowBar.create(host.querySelector('[data-fb]'), { hidden: true, state: 'listening', zoom: 2.2, seed: 11 }), cur: null };
    let z = 1, t0 = 0;
    const slide = (el, a) => { const e = eo(pr(t0, a, 220)); op(el, t0 >= a ? e : 0); el.style.transform = `translate3d(0, ${(10 * (1 - e)).toFixed(2)}px, 0)`; };
    const said = (el, t, a, at) => {
      op(el, fade(t, a, a + 1750));
      stream(el, t, a + 80, a + 1150);
      el.classList.toggle('cut', t >= a + 1200); el.classList.toggle('drop', t >= a + 1550);
      if (at) { const [x, y] = at; el.style.left = `${(x * cs + tx) * z}px`; el.style.top = `${(y * cs + ty) * z}px`; }
    };
    // The box between focus f and blur b: text streams in at a and is sent at a + 900. The caret sits after the
    // last visible word, stays solid while words land and blinks (500 ms on, 500 off) the rest of the time.
    const typed = (c, t, a, f, b) => {
      const fld = c.firstChild, txt = fld.lastChild, car = fld.firstChild, on = t >= a && t < a + 900, foc = t >= f && t < b;
      c.classList.toggle('has', on); fld.classList.toggle('in', on); c.classList.toggle('foc', foc);
      c.style.setProperty('--f', (foc ? eo(pr(t, f, 200)) : t < f ? 0 : 1 - pr(t, b, 50)).toFixed(3));
      stream(txt, t, a, a + 420);
      const ref = on ? (t < a + 420 ? t : a + 420) : t >= a + 900 ? a + 900 : f;
      car.style.visibility = foc && (t - ref) % 1000 < 500 ? 'visible' : 'hidden';
      if (!foc) return;
      const last = on && txt._w.filter(w => w.style.visibility === 'visible').pop(), ph = fld.children[1];
      const at = last || (on ? txt : ph), x = at.offsetLeft + (last ? last.offsetWidth + 1 : 0); // offsets are from the field
      const y = at.offsetTop;
      car.style.transform = `translate(${x}px, ${y}px)`;
    };
    // The pointer glides between PT stops, is an I-beam over a text box, presses on a click and hides while you type
    const pointer = t => {
      let i = 1; while (i < PT.length - 1 && t >= PT[i][0]) i++;
      const [a, x0, y0] = PT[i - 1], [b, x1, y1] = PT[i], k = eio(pr(t, a, b - a)), x = lerp(x0, x1, k), y = lerp(y0, y1, k);
      const over = (x > 318 && x < 560 && y > 892 && y < 920) || (x > 1020 && x < 1440 && y > 863 && y < 890);
      const p = Math.max(...CLICK.map(c => Math.sin(Math.PI * pr(t, c, 100)))), s = 1 - .1 * p;
      ptr.classList.toggle('ib', over);
      op(ptr, Math.max(...SHOWN.map(([u, v]) => fade(t, u, v, 120))));
      ptr.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px) scale(${s.toFixed(3)})`;
    };
    // The camera is one compositor animation, sampled 60 times a second from camAt. Chrome draws the layer once at
    // the deepest zoom and only moves that bitmap, so nothing redraws, softens or jumps mid-zoom, and a busy main
    // thread cannot make it stutter. frame() reads the same camAt for the bubble that follows the flow bar.
    const N = T / 1000 * 60, xf = ([cx, cy, s]) => `translate(${VW / 2 - cx * s}px, ${VH / 2 - cy * s}px) scale(${s})`;
    const anim = layer.animate(Array.from({ length: N + 1 }, (_, i) => ({ offset: i / N, transform: xf(camAt(cam.moves, T * i / N)) })), { duration: T, iterations: Infinity });
    let cs = 1, tx = 0, ty = 0;
    const frame = (t, wide) => {
      t0 = t;
      const [cx, cy, s] = wide ? [VW / 2, VH / 2, 1] : camAt(cam.moves, t);
      cs = s; tx = VW / 2 - cx * s; ty = VH / 2 - cy * s;
      op(g, Math.min(pr(t, 0, 300), 1 - pr(t, T - 350, 350)));
      const slackUp = t >= SK && t < HP, hpUp = t >= HP;
      tm.classList.toggle('idle', t >= SK); sk.classList.toggle('idle', !slackUp);
      app.textContent = hpUp ? 'Hushpen' : slackUp ? 'Slack' : 'Microsoft Teams';
      drive(fb, BAR, t);
      said(sayTm, t, L + 300, cam.at && cam.at[0]); said(saySk, t, SK + 300, cam.at && cam.at[1]);
      typed(tmCmp, t, L + 2100, TF, SK); typed(skCmp, t, SK + 2100, SK, HP);
      slide(tmNew, L + 3000); slide(skNew, SK + 3000);
      pointer(t);
      hp.style.opacity = pr(t, HP, 250).toFixed(3); // never hidden, so it is drawn ahead and fades in without a stall
      num.textContent = grouped(lerp(50692, 50707, eio(pr(t, HP + 600, 900))));
    };
    // One static zoom fits the screen to the box, so layout runs at the final size; the camera is one transform on top
    const fit = () => { z = host.clientWidth / VW; w.style.zoom = z; };
    return { T, still: HP + 1700, frame, fit, anim };
  };

  // The camera animation is the clock: every other part of the frame reads its time
  const mount = host => {
    const S = scene(host), A = S.anim;
    let t = 0, raf = 0;
    new ResizeObserver(() => { S.fit(); S.frame(t); }).observe(host);
    const tick = () => { t = (A.currentTime || 0) % S.T; S.frame(t); raf = requestAnimationFrame(tick); };
    S.fit(); S.frame(0); A.pause();
    const fix = new URLSearchParams(location.search).get('t');
    if (fix) { t = +fix % S.T; A.currentTime = t; S.frame(t); return; }
    // Reduce Motion: the whole desktop with Home, no zoom
    if (calm) { A.cancel(); t = S.still; S.frame(t, true); return; }
    new IntersectionObserver(([e]) => {
      if (e.isIntersecting) { A.play(); if (!raf) raf = requestAnimationFrame(tick); }
      else { A.pause(); cancelAnimationFrame(raf); raf = 0; }
    }, { threshold: .2 }).observe(host);
  };
  document.querySelectorAll('[data-wm]').forEach(mount);
})();
