// Why Hushpen, Free section: one camera on a soft macOS desktop. Safari downloads the release from GitHub,
// the repository names the AGPL-3.0 license, Hushpen opens straight to Home and takes a first dictation with
// no account, and Finder shows the recording in a folder on the computer. Windows keep their native size and
// sit at different depths, so every camera move has a soft parallax. The camera eases into each action and
// out again. Every frame is a pure function of the loop time, so the loop can start anywhere; it plays only
// while the box is on screen, and with Reduce Motion it shows one settled frame.
(() => {
  const host = document.querySelector('[data-wr]');
  if (!host) return;
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const W = 520, H = 390, CX = W / 2, CY = H / 2, T = 14000, STILL = 13150;
  const cl = x => (x < 0 ? 0 : x > 1 ? 1 : x);
  const pr = (t, a, d) => cl((t - a) / d);
  const lerp = (a, b, p) => a + (b - a) * p;
  // A cubic-bezier curve, sampled once. None of the curves overshoot, so nothing springs.
  const bez = (x1, y1, x2, y2) => {
    const N = 240, xs = new Float64Array(N + 1), ys = new Float64Array(N + 1);
    for (let i = 0; i <= N; i++) {
      const s = i / N, u = 1 - s;
      xs[i] = 3 * u * u * s * x1 + 3 * u * s * s * x2 + s * s * s;
      ys[i] = 3 * u * u * s * y1 + 3 * u * s * s * y2 + s * s * s;
    }
    return x => {
      x = cl(x);
      let lo = 0, hi = N;
      while (hi - lo > 1) { const m = (lo + hi) >> 1; if (xs[m] < x) lo = m; else hi = m; }
      const d = xs[hi] - xs[lo];
      return ys[lo] + (ys[hi] - ys[lo]) * (d ? (x - xs[lo]) / d : 0);
    };
  };
  const eo = bez(.22, 1, .36, 1);
  // A hand speeds up and slows down; the page's ease-out would start the pointer with a jump
  const glide = bez(.45, .05, .25, 1);

  // Style and class writes are cached on the element, so a frame only touches what changed
  const put = (el, k, v) => { if (el['_' + k] !== v) { el['_' + k] = v; el.style[k] = v; } };
  const flag = (el, c, on) => { if (el['_c' + c] !== on) { el['_c' + c] = on; el.classList.toggle(c, on); } };
  const n = (v, d = 2) => v.toFixed(d);
  const show = (el, o, y = 0, b = 0) => {
    put(el, 'opacity', n(cl(o), 3));
    put(el, 'transform', Math.abs(y) > .01 ? `translateY(${n(y)}px)` : 'none');
    put(el, 'filter', b > .05 ? `blur(${n(b)}px)` : 'none');
  };

  // GitHub Octicons (16 px, filled)
  const O = {
    menu: 'M1 2.75A.75.75 0 0 1 1.75 2h12.5a.75.75 0 0 1 0 1.5H1.75A.75.75 0 0 1 1 2.75Zm0 5A.75.75 0 0 1 1.75 7h12.5a.75.75 0 0 1 0 1.5H1.75A.75.75 0 0 1 1 7.75ZM1.75 12h12.5a.75.75 0 0 1 0 1.5H1.75a.75.75 0 0 1 0-1.5Z',
    search: 'M10.68 11.74a6 6 0 0 1-7.922-8.982 6 6 0 0 1 8.982 7.922l3.04 3.04a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215ZM11.5 7a4.499 4.499 0 1 0-8.997 0A4.499 4.499 0 0 0 11.5 7Z',
    code: 'm11.28 3.22 4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.275-.326.749.749 0 0 1 .215-.734L13.94 8l-3.72-3.72a.749.749 0 0 1 .326-1.275.749.749 0 0 1 .734.215Zm-6.56 0a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042L2.06 8l3.72 3.72a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L.47 8.53a.75.75 0 0 1 0-1.06Z',
    issue: 'M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z',
    pr: 'M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354ZM3.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm8.25.75a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Z',
    play: 'M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Zm4.879-2.773 4.264 2.559a.25.25 0 0 1 0 .428l-4.264 2.559A.25.25 0 0 1 6 10.559V5.442a.25.25 0 0 1 .379-.215Z',
    graph: 'M1.5 1.75V13.5h13.75a.75.75 0 0 1 0 1.5H.75a.75.75 0 0 1-.75-.75V1.75a.75.75 0 0 1 1.5 0Zm14.28 2.53-5.25 5.25a.75.75 0 0 1-1.06 0L7 7.06 4.28 9.78a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042l3.25-3.25a.75.75 0 0 1 1.06 0L10 7.94l4.72-4.72a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042Z',
    shield: 'M7.467.133a1.748 1.748 0 0 1 1.066 0l5.25 1.68A1.75 1.75 0 0 1 15 3.48V7c0 1.566-.32 3.182-1.303 4.682-.983 1.498-2.585 2.813-5.032 3.855a1.697 1.697 0 0 1-1.33 0c-2.447-1.042-4.049-2.357-5.032-3.855C1.32 10.182 1 8.566 1 7V3.48a1.755 1.755 0 0 1 1.217-1.667Zm.61 1.429a.25.25 0 0 0-.153 0l-5.25 1.68a.25.25 0 0 0-.174.238V7c0 1.358.275 2.666 1.057 3.86.784 1.194 2.121 2.34 4.366 3.297a.196.196 0 0 0 .154 0c2.245-.956 3.582-2.104 4.366-3.298C13.225 9.666 13.5 8.36 13.5 7V3.48a.251.251 0 0 0-.174-.237l-5.25-1.68Z',
    inbox: 'M2.8 2.06A1.75 1.75 0 0 1 4.41 1h7.18c.7 0 1.333.417 1.61 1.06l2.74 6.395c.04.093.06.194.06.295v4.5A1.75 1.75 0 0 1 14.25 15H1.75A1.75 1.75 0 0 1 0 13.25v-4.5c0-.101.02-.202.06-.295Zm1.61.44a.25.25 0 0 0-.23.152L1.887 8H4.75a.75.75 0 0 1 .6.3L6.625 10h2.75l1.275-1.7a.75.75 0 0 1 .6-.3h2.863L11.82 2.652a.25.25 0 0 0-.23-.152Zm10.09 7h-2.875l-1.275 1.7a.75.75 0 0 1-.6.3h-3.5a.75.75 0 0 1-.6-.3L4.375 9.5H1.5v3.75c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25Z',
    tag: 'M1 7.775V2.75C1 1.784 1.784 1 2.75 1h5.025c.464 0 .91.184 1.238.513l6.25 6.25a1.75 1.75 0 0 1 0 2.474l-5.026 5.026a1.75 1.75 0 0 1-2.474 0l-6.25-6.25A1.752 1.752 0 0 1 1 7.775Zm1.5 0c0 .066.026.13.073.177l6.25 6.25a.25.25 0 0 0 .354 0l5.025-5.025a.25.25 0 0 0 0-.354l-6.25-6.25a.25.25 0 0 0-.177-.073H2.75a.25.25 0 0 0-.25.25ZM6 5a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z',
    tri: 'm4.427 7.427 3.396 3.396a.25.25 0 0 0 .354 0l3.396-3.396A.25.25 0 0 0 11.396 7H4.604a.25.25 0 0 0-.177.427Z',
    pkg: 'm8.878.392 5.25 3.045c.54.314.872.89.872 1.514v6.098a1.75 1.75 0 0 1-.872 1.514l-5.25 3.045a1.75 1.75 0 0 1-1.756 0l-5.25-3.045A1.75 1.75 0 0 1 1 11.049V4.951c0-.624.332-1.201.872-1.514L7.122.392a1.75 1.75 0 0 1 1.756 0ZM7.875 1.69l-4.63 2.685L8 7.133l4.755-2.758-4.63-2.685a.248.248 0 0 0-.25 0ZM2.5 5.677v5.372c0 .09.047.171.125.216l4.625 2.683V8.432Zm6.25 8.271 4.625-2.683a.25.25 0 0 0 .125-.216V5.677L8.75 8.432Z',
    zip: 'M3.5 1.75v11.5c0 .09.048.173.126.217a.75.75 0 0 1-.752 1.298A1.748 1.748 0 0 1 2 13.25V1.75C2 .784 2.784 0 3.75 0h5.586c.464 0 .909.185 1.237.513l2.914 2.914c.329.328.513.773.513 1.237v8.586A1.75 1.75 0 0 1 12.25 15h-.5a.75.75 0 0 1 0-1.5h.5a.25.25 0 0 0 .25-.25V4.664a.25.25 0 0 0-.073-.177L9.513 1.573a.25.25 0 0 0-.177-.073H7.25a.75.75 0 0 1 0 1.5h-.5a.75.75 0 0 1 0-1.5h-3a.25.25 0 0 0-.25.25Zm3.75 8.75h.5c.966 0 1.75.784 1.75 1.75v3a.75.75 0 0 1-.75.75h-2.5a.75.75 0 0 1-.75-.75v-3c0-.966.784-1.75 1.75-1.75ZM6 5.25a.75.75 0 0 1 .75-.75h.5a.75.75 0 0 1 0 1.5h-.5A.75.75 0 0 1 6 5.25Zm.75 2.25h.5a.75.75 0 0 1 0 1.5h-.5a.75.75 0 0 1 0-1.5ZM8 6.75A.75.75 0 0 1 8.75 6h.5a.75.75 0 0 1 0 1.5h-.5A.75.75 0 0 1 8 6.75ZM8.75 3h.5a.75.75 0 0 1 0 1.5h-.5a.75.75 0 0 1 0-1.5ZM8 9.75A.75.75 0 0 1 8.75 9h.5a.75.75 0 0 1 0 1.5h-.5A.75.75 0 0 1 8 9.75Zm-1 2.5v2.25h1v-2.25a.25.25 0 0 0-.25-.25h-.5a.25.25 0 0 0-.25.25Z',
    branch: 'M9.5 3.25a2.25 2.25 0 1 1 3 2.122V6A2.5 2.5 0 0 1 10 8.5H6a1 1 0 0 0-1 1v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A2.493 2.493 0 0 1 6 7h4a1 1 0 0 0 1-1v-.628A2.25 2.25 0 0 1 9.5 3.25Zm-6 0a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Zm8.25-.75a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z',
    chev: 'M12.78 5.22a.749.749 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.06 0L3.22 6.28a.749.749 0 1 1 1.06-1.06L8 8.939l3.72-3.719a.749.749 0 0 1 1.06 0Z',
    hist: 'm.427 1.927 1.215 1.215a8.002 8.002 0 1 1-1.6 5.685.75.75 0 1 1 1.493-.154 6.5 6.5 0 1 0 1.18-4.458l1.358 1.358A.25.25 0 0 1 3.896 6H.25A.25.25 0 0 1 0 5.75V2.104a.25.25 0 0 1 .427-.177ZM7.75 4a.75.75 0 0 1 .75.75v2.992l2.028.812a.75.75 0 0 1-.557 1.392l-2.5-1A.751.751 0 0 1 7 8.25v-3.5A.75.75 0 0 1 7.75 4Z',
    dir: 'M1.75 1A1.75 1.75 0 0 0 0 2.75v10.5C0 14.216.784 15 1.75 15h12.5A1.75 1.75 0 0 0 16 13.25v-8.5A1.75 1.75 0 0 0 14.25 3H7.5a.25.25 0 0 1-.2-.1l-.9-1.2C6.07 1.26 5.55 1 5 1H1.75Z',
    file: 'M2 1.75C2 .784 2.784 0 3.75 0h6.586c.464 0 .909.184 1.237.513l2.914 2.914c.329.328.513.773.513 1.237v9.586A1.75 1.75 0 0 1 13.25 16h-9.5A1.75 1.75 0 0 1 2 14.25Zm1.75-.25a.25.25 0 0 0-.25.25v12.5c0 .138.112.25.25.25h9.5a.25.25 0 0 0 .25-.25V6h-2.75A1.75 1.75 0 0 1 9 4.25V1.5Zm6.75.062V4.25c0 .138.112.25.25.25h2.688l-.011-.013-2.914-2.914-.013-.011Z',
    book: 'M0 1.75A.75.75 0 0 1 .75 1h4.253c1.227 0 2.317.59 3 1.501A3.743 3.743 0 0 1 11.006 1h4.245a.75.75 0 0 1 .75.75v10.5a.75.75 0 0 1-.75.75h-4.507a2.25 2.25 0 0 0-1.591.659l-.622.621a.75.75 0 0 1-1.06 0l-.622-.621A2.25 2.25 0 0 0 5.258 13H.75a.75.75 0 0 1-.75-.75Zm7.251 10.324.004-5.073-.002-2.253A2.25 2.25 0 0 0 5.003 2.5H1.5v9h3.757a3.75 3.75 0 0 1 1.994.574ZM8.755 4.75l-.004 7.322a3.752 3.752 0 0 1 1.992-.572H14.5v-9h-3.495a2.25 2.25 0 0 0-2.25 2.25Z',
    law: 'M8.75.75V2h.985c.304 0 .603.08.867.231l1.29.736c.038.022.08.033.124.033h2.234a.75.75 0 0 1 0 1.5h-.427l2.111 4.692a.75.75 0 0 1-.154.838l-.53-.53.529.531-.001.002-.002.002-.006.006-.006.005-.01.01-.045.04c-.21.176-.441.327-.686.45C14.556 10.78 13.88 11 13 11a4.498 4.498 0 0 1-2.023-.454 3.544 3.544 0 0 1-.686-.45l-.045-.04-.016-.015-.006-.006-.004-.004v-.001a.75.75 0 0 1-.154-.838L12.178 4.5h-.162c-.305 0-.604-.079-.868-.231l-1.29-.736a.245.245 0 0 0-.124-.033H8.75V13h2.5a.75.75 0 0 1 0 1.5h-6.5a.75.75 0 0 1 0-1.5h2.5V3.5h-.984a.245.245 0 0 0-.124.033l-1.289.737c-.265.15-.564.23-.869.23h-.162l2.112 4.692a.75.75 0 0 1-.154.838l-.53-.53.529.531-.001.002-.002.002-.006.006-.016.015-.045.04c-.21.176-.441.327-.686.45C4.556 10.78 3.88 11 3 11a4.498 4.498 0 0 1-2.023-.454 3.544 3.544 0 0 1-.686-.45l-.045-.04-.016-.015-.006-.006-.004-.004v-.001a.75.75 0 0 1-.154-.838L2.178 4.5H1.75a.75.75 0 0 1 0-1.5h2.234a.249.249 0 0 0 .125-.033l1.288-.737c.265-.15.564-.23.869-.23h.984V.75a.75.75 0 0 1 1.5 0Zm2.945 8.477c.285.135.718.273 1.305.273s1.02-.138 1.305-.273L13 6.327Zm-10 0c.285.135.718.273 1.305.273s1.02-.138 1.305-.273L3 6.327Z',
    pulse: 'M6 2c.306 0 .582.187.696.471L10 10.731l1.304-3.26A.751.751 0 0 1 12 7h3.25a.75.75 0 0 1 0 1.5h-2.742l-1.812 4.528a.751.751 0 0 1-1.392 0L6 4.77 4.696 8.03A.75.75 0 0 1 4 8.5H.75a.75.75 0 0 1 0-1.5h2.742l1.812-4.529A.751.751 0 0 1 6 2Z',
    star: 'M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 .416 1.279l-3.046 2.97.719 4.192a.751.751 0 0 1-1.088.791L8 12.347l-3.766 1.98a.75.75 0 0 1-1.088-.79l.72-4.194L.818 6.374a.75.75 0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25Zm0 2.445L6.615 5.5a.75.75 0 0 1-.564.41l-3.097.45 2.24 2.184a.75.75 0 0 1 .216.664l-.528 3.084 2.769-1.456a.75.75 0 0 1 .698 0l2.77 1.456-.53-3.084a.75.75 0 0 1 .216-.664l2.24-2.183-3.096-.45a.75.75 0 0 1-.564-.41L8 2.694Z',
    eye: 'M8 2c1.981 0 3.671.992 4.933 2.078 1.27 1.091 2.187 2.345 2.637 3.023a1.62 1.62 0 0 1 0 1.798c-.45.678-1.367 1.932-2.637 3.023C11.67 13.008 9.981 14 8 14c-1.981 0-3.671-.992-4.933-2.078C1.797 10.83.88 9.576.43 8.898a1.62 1.62 0 0 1 0-1.798c.45-.677 1.367-1.931 2.637-3.022C4.33 2.992 6.019 2 8 2ZM1.679 7.932a.12.12 0 0 0 0 .136c.411.622 1.241 1.75 2.366 2.717C5.176 11.758 6.527 12.5 8 12.5c1.473 0 2.825-.742 3.955-1.715 1.124-.967 1.954-2.096 2.366-2.717a.12.12 0 0 0 0-.136c-.412-.621-1.242-1.75-2.366-2.717C10.824 4.242 9.473 3.5 8 3.5c-1.473 0-2.825.742-3.955 1.715-1.124.967-1.954 2.096-2.366 2.717ZM8 10a2 2 0 1 1-.001-3.999A2 2 0 0 1 8 10Z',
    plus: 'M7.75 2a.75.75 0 0 1 .75.75V7h4.25a.75.75 0 0 1 0 1.5H8.5v4.25a.75.75 0 0 1-1.5 0V8.5H2.75a.75.75 0 0 1 0-1.5H7V2.75A.75.75 0 0 1 7.75 2Z',
    copilot: 'M7.998 15.035c-4.562 0-7.873-2.914-7.998-3.749V9.338c.085-.628.677-1.686 1.588-2.065.013-.07.024-.143.036-.218.029-.183.06-.384.126-.612-.201-.508-.254-1.084-.254-1.656 0-.87.128-1.769.693-2.484.579-.733 1.494-1.124 2.724-1.261 1.206-.134 2.262.034 2.944.765.05.053.096.108.139.165.044-.057.094-.112.143-.165.682-.731 1.738-.899 2.944-.765 1.23.137 2.145.528 2.724 1.261.566.715.693 1.614.693 2.484 0 .572-.053 1.148-.254 1.656.066.228.098.429.126.612.012.076.024.148.037.218.924.385 1.522 1.471 1.591 2.095v1.872c0 .766-3.351 3.795-8.002 3.795Zm0-1.485c2.28 0 4.584-1.11 5.002-1.433V7.862l-.023-.116c-.49.21-1.075.291-1.727.291-1.146 0-2.059-.327-2.71-.991A3.222 3.222 0 0 1 8 6.303a3.24 3.24 0 0 1-.544.743c-.65.664-1.563.991-2.71.991-.652 0-1.236-.081-1.727-.291l-.023.116v4.255c.419.323 2.722 1.433 5.002 1.433ZM6.762 2.83c-.193-.206-.637-.413-1.682-.297-1.019.113-1.479.404-1.713.7-.247.312-.369.789-.369 1.554 0 .793.129 1.171.308 1.371.162.181.519.379 1.442.379.853 0 1.339-.235 1.638-.54.315-.322.527-.827.617-1.553.117-.935-.037-1.395-.241-1.614Zm4.155-.297c-1.044-.116-1.488.091-1.681.297-.204.219-.359.679-.242 1.614.091.726.303 1.231.618 1.553.299.305.784.54 1.638.54.922 0 1.28-.198 1.442-.379.179-.2.308-.578.308-1.371 0-.765-.123-1.242-.37-1.554-.233-.296-.693-.587-1.713-.7ZM6.25 9.037a.75.75 0 0 1 .75.75v1.501a.75.75 0 0 1-1.5 0V9.787a.75.75 0 0 1 .75-.75Zm4.25.75v1.501a.75.75 0 0 1-1.5 0V9.787a.75.75 0 0 1 1.5 0Z',
    cmdp: 'm6.354 8.04-4.773 4.773a.75.75 0 1 0 1.061 1.06L7.945 8.57a.75.75 0 0 0 0-1.06L2.642 2.206a.75.75 0 0 0-1.06 1.061L6.353 8.04ZM8.75 11.5a.75.75 0 0 0 0 1.5h5.5a.75.75 0 0 0 0-1.5h-5.5Z',
    fork: 'M5 5.372v.878c0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75v-.878a2.25 2.25 0 1 1 1.5 0v.878a2.25 2.25 0 0 1-2.25 2.25h-1.5v2.128a2.251 2.251 0 1 1-1.5 0V8.5h-1.5A2.25 2.25 0 0 1 3.5 6.25v-.878a2.25 2.25 0 1 1 1.5 0ZM5 3.25a.75.75 0 1 0-1.5 0 .75.75 0 0 0 1.5 0Zm6.75.75a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm-3 8.75a.75.75 0 1 0-1.5 0 .75.75 0 0 0 1.5 0Z',
  };
  const oc = (k, c = '') => `<svg class="oc${c ? ' ' + c : ''}" viewBox="0 0 16 16" aria-hidden="true"><path d="${O[k]}"/></svg>`;
  // The GitHub mark as github.com draws it now, in the header and as the site icon in the address field
  const GHP = 'M6.766 11.328c-2.063-.25-3.516-1.734-3.516-3.656 0-.781.281-1.625.75-2.188-.203-.515-.172-1.609.063-2.062.625-.078 1.468.25 1.968.703.594-.187 1.219-.281 1.985-.281.765 0 1.39.094 1.953.265.484-.437 1.344-.765 1.969-.687.218.422.25 1.515.046 2.047.5.593.766 1.39.766 2.203 0 1.922-1.453 3.375-3.547 3.64.531.344.89 1.094.89 1.954v1.625c0 .468.391.734.86.547C13.781 14.359 16 11.53 16 8.03 16 3.61 12.406 0 7.984 0 3.563 0 0 3.61 0 8.031a7.88 7.88 0 0 0 5.172 7.422c.422.156.828-.125.828-.547v-1.25c-.219.094-.5.156-.75.156-1.031 0-1.64-.562-2.078-1.609-.172-.422-.36-.672-.719-.719-.187-.015-.25-.093-.25-.187 0-.188.313-.328.625-.328.453 0 .844.281 1.25.86.313.452.64.655 1.031.655s.641-.14 1-.5c.266-.265.47-.5.657-.656';
  const GHM = `<svg class="oc wg-mk" viewBox="0 0 16 16" aria-hidden="true"><path d="${GHP}"/></svg>`;
  const GHF = `<svg class="oc sf-fav" viewBox="0 0 16 16" aria-hidden="true"><path d="${GHP}"/></svg>`;
  // A GitHub default avatar: a five by five identicon, mirrored
  const IDN = [[1, 0, 1], [0, 1, 1], [1, 1, 0], [0, 0, 1], [1, 0, 1]];
  const idn = (c = '') => `<svg class="idn${c ? ' ' + c : ''}" viewBox="0 0 6 6" aria-hidden="true"><rect width="6" height="6" fill="#F0F0F0"/>` +
    IDN.flatMap((r, y) => [0, 1, 2, 3, 4].filter(x => r[x < 3 ? x : 4 - x]).map(x => `<rect x="${.5 + x}" y="${.5 + y}" width="1.02" height="1.02" fill="#6FA8DC"/>`)).join('') + '</svg>';

  // Safari 26's own SF Symbols, the same set the hero's Safari uses, each cropped to its glyph
  const SF = {
    sidebar: ['2 1 18.42 14.39', '<path d="M7.95 14.37H9.18V2.03H7.95ZM5.3 15.39H17.32C19.28 15.39 20.41 14.19 20.41 12.08V4.3C20.41 2.2 19.28 1 17.32 1H5.3C3.2 1 2 2.2 2 4.3V12.08C2 14.19 3.2 15.39 5.3 15.39ZM5.3 14.12C4.01 14.12 3.25 13.38 3.25 12.08V4.31C3.25 3.01 4.01 2.27 5.3 2.27H17.11C18.42 2.27 19.16 3.01 19.16 4.31V12.08C19.16 13.38 18.42 14.12 17.11 14.12ZM6.45 5.16C6.69 5.16 6.89 4.95 6.89 4.72C6.89 4.48 6.69 4.29 6.45 4.29H4.78C4.54 4.29 4.34 4.48 4.34 4.72C4.34 4.95 4.54 5.16 4.78 5.16ZM6.45 7.19C6.69 7.19 6.89 6.98 6.89 6.74C6.89 6.51 6.69 6.31 6.45 6.31H4.78C4.54 6.31 4.34 6.51 4.34 6.74C4.34 6.98 4.54 7.19 4.78 7.19ZM6.45 9.21C6.69 9.21 6.89 9 6.89 8.77C6.89 8.54 6.69 8.34 6.45 8.34H4.78C4.54 8.34 4.34 8.54 4.34 8.77C4.34 9 4.54 9.21 4.78 9.21Z"/>'],
    down: ['2 1 13.57 7.64', '<path d="M8.79 8.64C8.98 8.64 9.18 8.56 9.31 8.41L15.36 2.22C15.49 2.09 15.57 1.92 15.57 1.72C15.57 1.31 15.26 1 14.86 1C14.67 1 14.48 1.08 14.34 1.21L8.37 7.31H9.2L3.22 1.21C3.09 1.08 2.92 1 2.72 1C2.31 1 2 1.31 2 1.72C2 1.92 2.08 2.09 2.21 2.22L8.26 8.41C8.41 8.56 8.58 8.64 8.79 8.64Z"/>'],
    left: ['1 1 7.64 13.55', '<path d="M1 7.78C1 7.97 1.08 8.14 1.22 8.3L7.41 14.35C7.54 14.48 7.73 14.55 7.93 14.55C8.33 14.55 8.64 14.25 8.64 13.84C8.64 13.64 8.56 13.47 8.43 13.34L2.75 7.78L8.43 2.22C8.56 2.08 8.64 1.9 8.64 1.71C8.64 1.3 8.33 1 7.93 1C7.73 1 7.54 1.07 7.41 1.21L1.22 7.26C1.08 7.41 1 7.58 1 7.78Z"/>'],
    right: ['3 1 7.64 13.55', '<path d="M10.64 7.78C10.64 7.58 10.57 7.41 10.41 7.26L4.22 1.21C4.09 1.07 3.92 1 3.72 1C3.31 1 3 1.3 3 1.71C3 1.9 3.09 2.08 3.21 2.22L8.89 7.78L3.21 13.34C3.09 13.47 3 13.64 3 13.84C3 14.25 3.31 14.55 3.72 14.55C3.92 14.55 4.09 14.48 4.22 14.35L10.41 8.3C10.57 8.14 10.64 7.97 10.64 7.78Z"/>'],
    share: ['2 2.2 13.86 17.57', '<path class="st" d="M6.75 7.86H5.27C3.55 7.86 2.63 8.77 2.63 10.48V16.5C2.63 18.22 3.55 19.14 5.27 19.14H12.59C14.31 19.14 15.23 18.22 15.23 16.5V10.48C15.23 8.77 14.31 7.86 12.59 7.86H11.1"/><path d="M6.28 5.87C6.43 5.87 6.6 5.81 6.71 5.69L7.92 4.4L8.93 3.32L9.94 4.4L11.14 5.69C11.26 5.81 11.41 5.87 11.57 5.87C11.89 5.87 12.14 5.64 12.14 5.33C12.14 5.15 12.07 5.03 11.96 4.91L9.38 2.43C9.22 2.27 9.09 2.22 8.93 2.22C8.77 2.22 8.64 2.27 8.48 2.43L5.9 4.91C5.78 5.03 5.72 5.15 5.72 5.33C5.72 5.64 5.96 5.87 6.28 5.87ZM8.93 13.65C9.27 13.65 9.55 13.38 9.55 13.05V5.1L9.45 3C9.44 2.71 9.21 2.48 8.93 2.48C8.64 2.48 8.43 2.71 8.41 3L8.31 5.1V13.05C8.31 13.38 8.59 13.65 8.93 13.65Z"/>'],
    plus: ['2 1 12.89 12.89', '<path d="M9.14 13.19V1.7C9.14 1.33 8.83 1 8.45 1C8.07 1 7.75 1.33 7.75 1.7V13.19C7.75 13.57 8.07 13.89 8.45 13.89C8.83 13.89 9.14 13.57 9.14 13.19ZM2.7 8.15H14.19C14.57 8.15 14.89 7.83 14.89 7.45C14.89 7.07 14.57 6.75 14.19 6.75H2.7C2.32 6.75 2 7.07 2 7.45C2 7.83 2.32 8.15 2.7 8.15Z"/>'],
    ovw: ['2 1 16.53 16.36', '<path class="st" d="M4.95 13.11H4.45C3.2 13.11 2.63 12.51 2.63 11.29V3.45C2.63 2.24 3.2 1.63 4.45 1.63H12.29C13.51 1.63 14.11 2.24 14.11 3.45V3.85"/><path d="M8.24 17.36H16.08C17.71 17.36 18.53 16.55 18.53 14.94V7.04C18.53 5.43 17.71 4.62 16.08 4.62H8.24C6.6 4.62 5.79 5.42 5.79 7.04V14.94C5.79 16.55 6.6 17.36 8.24 17.36ZM8.26 16.1C7.48 16.1 7.04 15.68 7.04 14.87V7.11C7.04 6.29 7.48 5.88 8.26 5.88H16.06C16.83 5.88 17.27 6.29 17.27 7.11V14.87C17.27 15.68 16.83 16.1 16.06 16.1Z"/>'],
    menu: ['0 0 15 15', '<rect class="st" x="1.3" y="1" width="12.4" height="6.4" rx="1.8"/><path class="st" d="M1.6 10.5h11.8M1.6 13.7h8"/>'],
  };
  const sfi = k => `<svg class="sf-i sf-${k}" viewBox="${SF[k][0]}" aria-hidden="true">${SF[k][1]}</svg>`;
  // Finder and menu bar glyphs in the SF Symbols style (24 px, stroked)
  const I = {
    back: '<path d="m14.5 6-6 6 6 6"/>',
    fwd: '<path d="m9.5 6 6 6-6 6"/>',
    share: '<path d="M12 3.5v11M8.5 7 12 3.5 15.5 7M8 10.5H6.5A1.5 1.5 0 0 0 5 12v6.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V12a1.5 1.5 0 0 0-1.5-1.5H16"/>',
    xc: '<circle cx="12" cy="12" r="8.6"/><path d="m9.3 9.3 5.4 5.4M14.7 9.3l-5.4 5.4"/>',
    mag: '<circle cx="12" cy="12" r="8.6"/><circle cx="11.3" cy="11.3" r="3.2"/><path d="m13.6 13.6 2.6 2.6"/>',
    recents: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>',
    apps: '<path d="M6 19.5 12 4.5l6 15M8.4 14h7.2"/>',
    desk: '<rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="M3 8.5h18M9 16.5h6"/>',
    doc: '<path d="M6.5 3h7l4 4v13.5h-11zM13.5 3v4h4"/>',
    dlc: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.6v8.2M8.6 12.6l3.4 3.4 3.4-3.4"/>',
    hd: '<rect x="3" y="7" width="18" height="10" rx="2.5"/><path d="M3.5 13h17M17 15.1h.01"/>',
    home: '<path d="M4.5 10.5 12 4l7.5 6.5V20h-15zM10 20v-5h4v5"/>',
    list: '<path d="M9 6.5h11M9 12h11M9 17.5h11M4.5 6.5h.01M4.5 12h.01M4.5 17.5h.01"/>',
    grid: '<rect x="4" y="4" width="7" height="7" rx="1.5"/><rect x="13" y="4" width="7" height="7" rx="1.5"/><rect x="4" y="13" width="7" height="7" rx="1.5"/><rect x="13" y="13" width="7" height="7" rx="1.5"/>',
    stag: '<path d="M3.5 4.5v6.3c0 .5.2 1 .6 1.4l7.7 7.7c.8.8 2 .8 2.8 0l5.3-5.3c.8-.8.8-2 0-2.8L12.2 4.1c-.4-.4-.9-.6-1.4-.6H4.5a1 1 0 0 0-1 1Z"/><circle cx="8" cy="8" r="1.3"/>',
    ssearch: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 4.5 4.5"/>',
    dch: '<path d="M9 5l7 7-7 7"/>',
    wifi: '<path d="M2.6 9a14 14 0 0 1 18.8 0M5.9 12.4a9.3 9.3 0 0 1 12.2 0M9.2 15.8a4.6 4.6 0 0 1 5.6 0"/><circle cx="12" cy="19.2" r="1.3" fill="currentColor" stroke="none"/>',
    cc: '<rect x="3" y="4.5" width="18" height="6" rx="3"/><circle cx="17.5" cy="7.5" r="1.6"/><rect x="3" y="13.5" width="18" height="6" rx="3"/><circle cx="6.5" cy="16.5" r="1.6"/>',
  };
  const ic = (k, c = '') => `<svg class="sf${c ? ' ' + c : ''}" viewBox="0 0 24 24" aria-hidden="true">${I[k]}</svg>`;
  const CUR = '<svg class="wf-cur" viewBox="0 0 19 28" aria-hidden="true"><path d="M2 2v20.5l5-4.7 3.2 7.4 3.4-1.5-3.2-7.3H17z" stroke-linejoin="round"/></svg>';
  const TL = (c = '') => `<span class="tl${c ? ' ' + c : ''}"><i></i><i></i><i></i></span>`;
  const FOLD = '<svg class="fi" viewBox="0 0 20 16" aria-hidden="true"><path d="M1 3.3C1 2.6 1.6 2 2.3 2h4.4c.4 0 .8.2 1 .5L9 4h8.7c.7 0 1.3.6 1.3 1.3v8.4c0 .7-.6 1.3-1.3 1.3H2.3C1.6 15 1 14.4 1 13.7z" fill="#58A6EC"/><path d="M1 6.2C1 5.5 1.6 5 2.3 5h15.4c.7 0 1.3.5 1.3 1.2v7.5c0 .7-.6 1.3-1.3 1.3H2.3C1.6 15 1 14.4 1 13.7z" fill="#86C6FA"/><path d="M1.8 5.7h16.4" stroke="#B9DEFD" stroke-width=".6"/></svg>';
  const WAV = '<svg class="fi" viewBox="0 0 20 16" aria-hidden="true"><path d="M5 .6h6.6L15 4v11.4H5z" fill="#fff" stroke="#BDBDC2" stroke-width=".7"/><path d="M11.6.6V4H15" fill="#EEEEF0" stroke="#BDBDC2" stroke-width=".7"/><path d="M7.4 9.2v1.6M9 8v4M10.6 7v6M12.2 8.6v2.8" stroke="#FA2D48" stroke-width=".9" stroke-linecap="round"/></svg>';
  const DMG = '<svg class="dmg" viewBox="0 0 32 32" aria-hidden="true"><rect x="4" y="9" width="24" height="15" rx="3.5" fill="#F4F4F6" stroke="#B4B4BA" stroke-width=".8"/><path d="M4.4 19.6h23.2" stroke="#CBCBD0" stroke-width=".8"/><circle cx="24" cy="21.9" r=".9" fill="#6FCF64"/></svg>';

  // Safari on GitHub: the release page, then the repository
  let bytes = 0;
  const ASSETS = [['macos-dmg', 'Mac disk image', 'mac'], ['linux-x64-appimage', 'Linux x64 AppImage'], ['linux-arm64-appimage', 'Linux arm64 AppImage']];
  const RFILES = [['crates', 'Reuse the Linux audio server connection', '2 days ago'], ['docs', 'Note the voice mix in the manual', '4 days ago'], ['harness', 'Reuse the native checkout build cache', '3 days ago'],
    ['packaging', 'Reuse the cached AppImage runtime on lin1', '2 days ago'], ['tests', 'Cover the read aloud queue', '5 days ago'], ['LICENSE', 'Add the license', 'last year', 1], ['README.md', 'Show the latest downloads', '2 days ago', 1]];
  const TABS = [['code', 'Code', 0, 'wg-code'], ['issue', 'Issues', 3], ['pr', 'Pull requests'], ['play', 'Actions'], ['shield', 'Security'], ['graph', 'Insights']];
  // Safari 26 with the compact tab bar, as in the hero: one tab that is also the address field, and the
  // downloads button in its own capsule before share, new tab, and the tab overview
  const SAF = `<div class="sf-tb ws-tb">${TL()}<span class="sf-cap sf-sb">${sfi('sidebar')}${sfi('down')}</span><span class="sf-cap sf-nv">${sfi('left')}<i></i>${sfi('right')}</span>` +
    `<div class="sf-tabs"><div class="sf-at">${GHF}<span>github.com</span>${sfi('menu')}</div></div>` +
    `<span class="sf-cap ws-dl"><svg class="sf" viewBox="0 0 24 24" aria-hidden="true"><circle class="tr" cx="12" cy="12" r="8.6"/><circle class="pg" cx="12" cy="12" r="8.6" pathLength="100" transform="rotate(-90 12 12)"/><path d="M12 7.6v8.2M8.6 12.6l3.4 3.4 3.4-3.4"/></svg></span>` +
    `<span class="sf-cap sf-rt">${sfi('share')}${sfi('plus')}${sfi('ovw')}</span></div>` +
    `<div class="ws-pg"><div class="wg-hd"><div class="wg-top"><div class="wg-s"><span class="wg-bt">${oc('menu')}</span>${GHM}<span class="wg-cr"><a>surajmandalcell</a><i>/</i><a><b>hushpen</b></a></span></div>` +
    `<div class="wg-e"><span class="wg-sr">${oc('search')}<span>Type <kbd>/</kbd> to search</span><i>${oc('cmdp')}</i></span><span class="wg-cp"><span class="wg-bt">${oc('copilot')}</span><span class="wg-bt">${oc('tri')}</span></span>` +
    `<span class="wg-ac"><span class="wg-bt wg-new">${oc('plus')}${oc('tri')}</span><span class="wg-bt">${oc('issue')}</span><span class="wg-bt">${oc('pr')}</span></span><span class="wg-bt">${oc('inbox')}</span>${idn('wg-av')}</div></div>` +
    `<div class="wg-tabs">${TABS.map(([i, t, c, k]) => `<span class="${k ? 'on ' + k : ''}"><b>${oc(i)}${t}${c ? `<em>${c}</em>` : ''}</b></span>`).join('')}</div></div>` +
    `<div class="wg-pg wg-rel"><p class="wg-bc"><a>Releases</a><i>/</i><b data-wf-tag>Latest version</b></p><div class="wg-card"><div class="wg-ch"><h4 data-wf-title>Latest version</h4><span class="wg-lt">Latest</span></div>` +
    `<p class="wg-meta">${idn()}<b>surajmandalcell</b><span data-wf-published></span><span class="wg-tg">${oc('tag')}<span data-wf-tag>Latest version</span></span></p><p class="wg-nt" hidden></p>` +
    `<h5>${oc('tri')}Assets<em data-wf-count></em></h5><div class="wg-as">${ASSETS.map(([kind, nm, c]) => `<div class="wg-a${c ? ' ' + c : ''}" data-wf-asset="${kind}">${oc('pkg')}<b>${nm}</b><em></em><time></time></div>`).join('')}</div>` +
    `<p class="wg-all">Show all assets</p></div></div>` +
    `<div class="wg-pg wg-rp"><div class="wg-tt">${idn()}<b>hushpen</b><span class="wg-pub">Public</span><span class="wg-acts"><span class="wg-btn">${oc('eye')}Watch<em>3</em>${oc('chev', 'cv')}</span>` +
    `<span class="wg-btn">${oc('fork')}Fork<em>4</em>${oc('chev', 'cv')}</span><span class="wg-btn">${oc('star')}Star<em>56</em>${oc('chev', 'cv')}</span></span></div><div class="wg-cols"><div class="wg-mn">` +
    `<div class="wg-bar"><span class="wg-btn">${oc('branch')}master${oc('chev', 'cv')}</span><span class="wg-btn wg-fd">${oc('search')}Go to file</span><span class="wg-btn go">${oc('code')}Code${oc('chev', 'cv')}</span></div>` +
    `<div class="wg-fl"><div class="wg-fh">${idn()}<b>surajmandalcell</b><span>Reuse the cached AppImage runtime on lin1</span><span class="ct">${oc('hist')}1,186 Commits</span></div>` +
    `${RFILES.map(([nm, m, d, f]) => `<div class="wg-fr">${oc(f ? 'file' : 'dir', f ? 'f' : 'd')}<b>${nm}</b><span class="m">${m}</span><time>${d}</time></div>`).join('')}</div></div>` +
    `<div class="wg-ab"><h6>About</h6><p>Private dictation for Mac and Linux. Speech turns into text on your computer.</p><div class="wg-ls">` +
    `<span class="wg-ln">${oc('book')}<span>Readme</span></span><span class="wg-ln wg-lic"><i class="tint"></i>${oc('law')}<span>AGPL-3.0 license</span></span><span class="wg-ln">${oc('pulse')}<span>Activity</span></span>` +
    `<span class="wg-ln">${oc('star')}<span><b>56</b> stars</span></span><span class="wg-ln">${oc('eye')}<span><b>3</b> watching</span></span><span class="wg-ln">${oc('fork')}<span><b>4</b> forks</span></span></div>` +
    `<div class="wg-rl"><h6>Releases</h6><p class="wg-rv">${oc('tag')}<b data-wf-title>Latest version</b><span class="wg-lt">Latest</span></p></div></div></div></div></div>` +
    `<div class="ws-pop"><div class="drow">${DMG}<span class="nm"><b data-wf-file>Hushpen.dmg</b><i class="dbar"><b></b></i><span class="st">Downloading</span></span><span class="ac">${ic('xc', 'xc')}${ic('mag', 'dmag')}</span></div><div class="ft">Clear</div></div>` +
    `<div class="ws-fly">${DMG}</div>`;

  // Hushpen is the real Home screenshot. The rows under the notice move up, because with a speech model
  // in place Home has no notice; the status, the button and the transcript are drawn over it in place.
  const HP = `<div class="hh-sb"><i class="hh-img"></i><b class="hh-nm">Hushpen</b></div><div class="hh-ct"><i class="hh-a"></i><i class="hh-b"></i></div>` +
    `<div class="hh-row"><span class="hh-st"><span class="ok">Ready</span><span>Listening</span><span>Transcribing</span><span>Done</span></span>` +
    `<span class="hh-cp">Copy</span><span class="hh-rec"><span>Record</span><span>Stop</span></span></div>` +
    `<div class="hh-tx"><span>Speak now. Stop when you are done.</span><span>Transcribing your recording.</span><span class="said">Hi Elena, the draft is ready. See you at ten.</span></div>${TL('nz hh-tl')}`;

  // Finder on the Hushpen data folder
  const FAV = [['recents', 'Recents'], ['apps', 'Applications'], ['desk', 'Desktop'], ['doc', 'Documents'], ['dlc', 'Downloads']];
  const FROWS = [['audio', 'Today at 9:41 AM', '--', 'Folder', 'open'], ['1042.wav', 'Today at 9:41 AM', '212 KB', 'Waveform audio', 'kid new'], ['1041.wav', 'Today at 9:38 AM', '486 KB', 'Waveform audio', 'kid'],
    ['cache', 'Today at 9:41 AM', '--', 'Folder'], ['config', 'Yesterday at 6:02 PM', '--', 'Folder'], ['history', 'Today at 9:41 AM', '--', 'Folder'], ['logs', 'Today at 9:41 AM', '--', 'Folder'], ['models', 'May 12, 2026 at 10:15 AM', '--', 'Folder'], ['voices', 'May 12, 2026 at 10:20 AM', '--', 'Folder']];
  const frow = ([nm, d, s, k, c = '']) => {
    const kid = c.includes('kid'), row = `<div class="wn-r${c.includes('open') ? ' open' : ''}${kid ? ' kid' : ''}"><span class="nm">${kid ? '' : ic('dch', 'dc')}${kid ? WAV : FOLD}<b>${nm}</b></span><span>${d}</span><span>${s}</span><span>${k}</span></div>`;
    return c.includes('new') ? `<div class="wn-new">${row}</div>` : row;
  };
  const PATH = [['hd', 'Macintosh HD', 'hd'], ['f', 'Users'], ['home', 'elena'], ['f', 'Library'], ['f', 'Application Support'], ['f', 'Hushpen'], ['f', 'audio', 'fl'], ['w', '1042.wav', 'fl']];
  const FN = `<div class="wn-sb">${TL()}<small>Favorites</small>${FAV.map(([i, t]) => `<span>${ic(i)}${t}</span>`).join('')}<small>Locations</small><span class="g">${ic('hd')}Macintosh HD</span></div>` +
    `<div class="wn-ct"><div class="wn-tb"><span class="wn-nv">${ic('back')}${ic('fwd', 'off')}</span><b>Hushpen</b><span class="wn-tr">${ic('list')}${ic('grid')}${ic('share')}${ic('stag')}${ic('ssearch')}</span></div>` +
    `<div class="wn-hd"><span>Name</span><span>Date Modified</span><span>Size</span><span>Kind</span></div><div class="wn-ls">${FROWS.map(frow).join('')}</div></div>` +
    `<div class="wn-bt"><div class="wn-pb">${PATH.map(([i, t, c = ''], j) => `${j ? ic('dch', 'sep' + (c === 'fl' ? ' fl' : '')) : ''}<span class="${c}">${i === 'f' ? FOLD : i === 'w' ? WAV : ic(i)}${t}</span>`).join('')}</div><div class="wn-stb">7 items, 412.8 GB available</div></div>`;

  // The menu bar names the app in front; the Hushpen glyph shows while Hushpen runs
  const APPS = [['Safari', 'File Edit View History Bookmarks Window Help'], ['Hushpen', 'Edit'], ['Finder', 'File Edit View Go Window Help']];
  const MB = `<svg class="mb-ap" viewBox="0 0 24 24" aria-hidden="true"><use href="#i-apple"/></svg>${APPS.map(([a, m]) => `<span class="mb-g"><b>${a}</b>${m.split(' ').map(s => `<span>${s}</span>`).join('')}</span>`).join('')}` +
    `<span class="mb-rt"><img class="mb-hp" src="assets/hushpen-menubar.svg" alt="">${ic('wifi')}${ic('cc')}<span>Thu May 14&nbsp;&nbsp;9:41 AM</span></span>`;

  // Layers: native size, world place and scale, and depth (under 1 is farther, so it moves and grows less)
  const K = .42;
  const L = {
    bg: { w: 780, h: 590, x: -130, y: -100, k: 1, d: .38, cls: 'wf-bg', html: '' },
    lt: { w: 780, h: 590, x: -130, y: -100, k: 1, d: .62, cls: 'wf-lt', html: '' },
    mb: { w: W / K, h: 24, x: 0, y: 0, k: K, d: .8, cls: 'wf-mb', html: MB },
    saf: { w: 980, h: 640, x: 20, y: 22, k: K, d: .9, cls: 'wf-w wf-saf', html: SAF },
    hp: { w: 940, h: 600, x: 106, y: 82, k: K, d: 1, cls: 'wf-w wf-hp', html: HP },
    fn: { w: 760, h: 440, x: 34, y: 188, k: K, d: 1.1, cls: 'wf-w wf-fn', html: FN },
  };
  const WINS = ['saf', 'hp', 'fn'];
  host.innerHTML = Object.keys(L).map(k => `<div class="${L[k].cls}">${L[k].html}</div>`).join('') + CUR;
  const q = s => host.querySelector(s);
  Object.keys(L).forEach(k => { L[k].el = host.querySelector(`:scope > .${L[k].cls.split(' ').pop()}`); });
  const cur = q('.wf-cur');
  const S = L.saf.el, HPW = L.hp.el, F = L.fn.el;
  const E = {
    dl: S.querySelector('.ws-dl'), ring: S.querySelector('.ws-dl .pg'), pop: S.querySelector('.ws-pop'), prow: S.querySelector('.ws-pop .drow'), pb: S.querySelector('.ws-pop .dbar'), pbf: S.querySelector('.ws-pop .dbar b'), pst: S.querySelector('.ws-pop .st'),
    fly: S.querySelector('.ws-fly'), mac: S.querySelector('.wg-a.mac'), code: S.querySelector('.wg-code'), rel: S.querySelector('.wg-rel'), rp: S.querySelector('.wg-rp'), lic: S.querySelector('.wg-lic'), tint: S.querySelector('.wg-lic .tint'),
    st: [...HPW.querySelectorAll('.hh-st > span')], cp: HPW.querySelector('.hh-cp'), rec: HPW.querySelector('.hh-rec'), recl: [...HPW.querySelectorAll('.hh-rec > span')], tx: [...HPW.querySelectorAll('.hh-tx > span')],
    nw: F.querySelector('.wn-new'), nwr: F.querySelector('.wn-new .wn-r'), hd: F.querySelector('.wn-pb span.hd'), fl: [...F.querySelectorAll('.wn-pb .fl')],
    mg: [...L.mb.el.querySelectorAll('.mb-g')], mhp: L.mb.el.querySelector('.mb-hp'),
  };

  // A point inside a window, in its native pixels. Offsets ignore transforms; the ratio undoes any zoom.
  const at = (wk, sel, fx = .5, fy = .5, dx = 0, dy = 0) => {
    const w = L[wk], el = w.el.querySelector(sel), r = w.el.offsetWidth / w.w || 1;
    let x = 0, y = 0;
    for (let e = el; e && e !== w.el; e = e.offsetParent) { x += e.offsetLeft; y += e.offsetTop; }
    return [(x + el.offsetWidth * fx) / r + dx, (y + el.offsetHeight * fy) / r + dy];
  };

  // Places in the Hushpen screenshot, in its native pixels
  const REC = [871, 74.5], ST = [790, 74.5], TXS = [603, 121];

  // The camera: a world point at the view center and a zoom. A layer of depth d sees the zoom raised to d
  // and a pan scaled by d. A shot names a point of one window, how big its native pixels look on the stage,
  // and where on the stage that point sits. A monotone cubic runs through the shots, so the camera never
  // stops between a move and the slow drift after it, and never overshoots a shot.
  let CAM = null, PK = [], PRESS = [];
  const spline = (ts, vs) => {
    const c = ts.length, m = new Float64Array(c), d = [];
    for (let i = 0; i < c - 1; i++) d.push((vs[i + 1] - vs[i]) / (ts[i + 1] - ts[i]));
    for (let i = 1; i < c - 1; i++) {
      if (d[i - 1] * d[i] <= 0) continue;
      const h0 = ts[i] - ts[i - 1], h1 = ts[i + 1] - ts[i];
      m[i] = 3 * (h0 + h1) / ((2 * h1 + h0) / d[i - 1] + (h1 + 2 * h0) / d[i]);
    }
    return t => {
      if (t <= ts[0]) return vs[0];
      if (t >= ts[c - 1]) return vs[c - 1];
      let i = 0;
      while (t > ts[i + 1]) i++;
      const h = ts[i + 1] - ts[i], s = (t - ts[i]) / h, s2 = s * s, s3 = s2 * s;
      return (2 * s3 - 3 * s2 + 1) * vs[i] + (s3 - 2 * s2 + s) * h * m[i] + (3 * s2 - 2 * s3) * vs[i + 1] + (s3 - s2) * h * m[i + 1];
    };
  };
  const measure = () => {
    const keys = [];
    const raw = (t, x, y, z) => keys.push([t, x, y, Math.log(z)]);
    const aim = (t, wk, p, ns, px = CX, py = CY) => {
      const w = L[wk], zd = ns / w.k, z = zd ** (1 / w.d);
      const fx = w.x + p[0] * w.k - (px - CX) / zd, fy = w.y + p[1] * w.k - (py - CY) / zd;
      keys.push([t, CX + (fx - CX) / w.d, CY + (fy - CY) / w.d, Math.log(z)]);
    };
    // Close shots keep an edge of their window in view, so the desktop color frames the action
    const dmg = at('saf', '.wg-a.mac b', .3, .55), dmgC = [170, dmg[1]], pop = at('saf', '.ws-pop', .5, .42), code = at('saf', '.wg-code b', .55, .55);
    const lic = at('saf', '.wg-lic > span', .45, .5), licC = [L.saf.w + 30 - CX / 1.4, lic[1]];
    const nw = at('fn', '.wn-new .wn-r', 0, 0, 150, 12), nwp = at('fn', '.wn-new .wn-r b', .6, 0, 0, 12);
    const pbR = at('fn', '.wn-pb span.fl:last-child', 0, .5, -60), pbL = at('fn', '.wn-pb span.hd', 0, .5, 150);
    const SAFW = [490, 318], SAFM = [430, 300];
    aim(0, 'saf', SAFW, .47);
    aim(700, 'saf', SAFW, .475);
    aim(1500, 'saf', dmgC, 1.3, CX, 198);
    aim(1950, 'saf', dmgC, 1.33, CX, 198);
    aim(2700, 'saf', pop, 1.36, CX, 182);
    aim(3450, 'saf', pop, 1.4, CX, 182);
    aim(4150, 'saf', SAFM, .58);
    aim(4500, 'saf', SAFM, .585);
    aim(5150, 'saf', licC, 1.4, CX, 196);
    aim(5950, 'saf', licC, 1.45, CX, 196);
    aim(6800, 'hp', [470, 300], .52);
    aim(7450, 'hp', ST, 1.52, CX, 178);
    aim(8250, 'hp', ST, 1.56, CX, 178);
    // The transcript shot ends just right of the status word, before the Copy button
    aim(9000, 'hp', TXS, 1.53, CX, 212);
    aim(9550, 'hp', TXS, 1.557, CX, 214);
    aim(10400, 'fn', [380, 220], .62);
    aim(11000, 'fn', nw, 1.45, CX, 182);
    aim(11400, 'fn', nw, 1.47, CX, 182);
    aim(11850, 'fn', pbR, 1.58, CX, 226);
    aim(12400, 'fn', pbL, 1.6, CX, 226);
    raw(13050, CX, CY + 2, 1);
    raw(13400, CX, CY + 1, .995);
    aim(T, 'saf', SAFW, .47);
    const ts = keys.map(k => k[0]);
    CAM = [1, 2, 3].map(i => spline(ts, keys.map(k => k[i])));
    // The pointer path: [t, window, x, y] in native pixels of that window. It moves in screen space, so it
    // can travel from one window to the next.
    // After Stop the pointer leaves the shot downward, clear of the transcript and the controls
    const rec = REC, away = [900, 340];
    PK = [[0, 'saf', 700, 520], [500, 'saf', 700, 520], [1550, 'saf', ...dmg], [3600, 'saf', ...dmg], [4150, 'saf', ...code], [4480, 'saf', ...code], [5050, 'saf', ...lic], [6300, 'saf', ...lic],
      [7250, 'hp', ...rec], [8550, 'hp', ...rec], [9050, 'hp', ...away], [10600, 'fn', ...nwp], [11120, 'fn', ...nwp], [11650, 'fn', nwp[0] + 70, nwp[1] + 92], [13300, 'fn', nwp[0] + 70, nwp[1] + 92], [T, 'saf', 700, 520]];
    PRESS = [1700, 4330, 7380, 8450, 10970];
    E.from = dmg; E.to = at('saf', '.ws-dl', .5, .5);
  };

  // Each window's own state: [opacity, scale about its center, rise in world px, blur in screen px, inactive]
  const state = (k, t) => {
    const back = eo(pr(t, 12450, 600)) * (1 - pr(t, 13300, 1));
    if (k === 'saf') {
      const a = eo(pr(t, 6400, 700)), b = eo(pr(t, 10050, 700)), r = eo(pr(t, 13300, 500));
      const blur = (2 * a + 1 * b) * (1 - back) * (1 - r);
      return [1, 1 - .02 * a * (1 - r), 0, blur, t >= 6420 && t < 13320];
    }
    const [o0, c0] = k === 'hp' ? [6350, 13300] : [10000, 13250];
    const e = eo(pr(t, o0, 520)), c = eo(pr(t, c0, 380));
    const o = cl(e * 2.2) * (1 - c), s = (.94 + .06 * e) * (1 - .05 * c);
    const blur = k === 'hp' ? 1.6 * eo(pr(t, 10050, 700)) * (1 - back) : 0;
    return [o, s, 10 * (1 - e), blur, k === 'hp' && t >= 10020];
  };

  const scene = t => {
    // Safari: the click, the file flying to the download button, the download, then the repository
    flag(E.mac, 'hov', t >= 1450 && t < 3700); flag(E.mac, 'ul', t >= 1600 && t < 2400);
    // The downloads button swells once as the file lands on it
    const land = Math.sin(Math.PI * pr(t, 2200, 300));
    put(E.dl, 'transform', land > .001 ? `scale(${n(1 + .12 * land, 3)})` : 'none');
    const g = glide(pr(t, 2300, 1100));
    put(E.ring, 'strokeDashoffset', n(100 * (1 - g), 1)); put(E.ring, 'opacity', n(1 - pr(t, 3420, 240), 3));
    const fp = pr(t, 1760, 560);
    if (fp > 0 && fp < 1) {
      const e = glide(fp), x = lerp(E.from[0], E.to[0], e), y = lerp(E.from[1], E.to[1], e) - 120 * Math.sin(Math.PI * e);
      put(E.fly, 'opacity', n(cl(fp * 8) * (1 - pr(fp, .82, .18)), 3)); put(E.fly, 'transform', `translate(${n(x - 16)}px, ${n(y - 16)}px) scale(${n(1.15 - .55 * e, 3)})`);
    } else put(E.fly, 'opacity', '0');
    const pi = eo(pr(t, 2100, 320)), po = pi * (1 - pr(t, 3850, 220));
    put(E.pop, 'visibility', po > 0 ? 'visible' : 'hidden'); put(E.pop, 'opacity', n(po, 3)); put(E.pop, 'transform', `translateY(${n(-6 * (1 - pi))}px)`);
    put(E.pbf, 'transform', `scaleX(${n(g, 4)})`); put(E.pb, 'opacity', n(1 - pr(t, 3400, 200), 3));
    const size = window.HushpenReleases.size;
    const s = bytes ? g < 1 ? `${size(bytes * g) || '0 MB'} of ${size(bytes)}` : size(bytes) : 'Downloading';
    if (E.pst._t !== s) { E.pst._t = s; E.pst.textContent = s; }
    flag(E.prow, 'done', g >= 1);
    // One page goes before the next comes, so the two never show through each other
    flag(E.code, 'hov', t >= 4100 && t < 4460);
    const ri = eo(pr(t, 4470, 420));
    show(E.rel, (1 - pr(t, 4340, 130)) + pr(t, 13500, 300)); show(E.rp, ri * (1 - pr(t, 13320, 170)), 10 * (1 - ri));
    put(E.rel, 'visibility', t < 4470 || t >= 13500 ? 'visible' : 'hidden'); put(E.rp, 'visibility', t >= 4470 && t < 13490 ? 'visible' : 'hidden');
    flag(E.lic, 'hov', t >= 5000 && t < 13350);
    put(E.tint, 'transform', `scaleX(${n(eo(pr(t, 5120, 560)), 4)})`); put(E.tint, 'opacity', n(1 - pr(t, 13350, 250), 3));

    // Hushpen: Record, Listening, Transcribing, then Done with the words, all in place
    const P = [7380, 8450, 8860];
    const ph = t < P[0] ? 0 : t < P[1] ? 1 : t < P[2] ? 2 : 3;
    E.st.forEach((el, i) => put(el, 'opacity', i === ph ? '1' : '0'));
    E.recl.forEach((el, i) => put(el, 'opacity', (ph === 1) === (i === 1) ? '1' : '0'));
    flag(E.rec, 'dn', PRESS.slice(2, 4).some(a => t >= a - 50 && t < a + 110));
    const cp = eo(pr(t, P[2], 260));
    put(E.cp, 'opacity', n(cp, 3)); put(E.cp, 'visibility', cp > 0 ? 'visible' : 'hidden');
    flag(HPW, 'done', t >= P[2]); flag(HPW, 'on', ph >= 1);
    show(E.tx[0], ph === 1 ? 1 : 0); show(E.tx[1], ph === 2 ? 1 : 0);
    const sp = eo(pr(t, P[2] + 60, 520));
    show(E.tx[2], sp, 6 * (1 - sp), 4 * (1 - sp));

    // Finder: the new recording lands in the audio folder and is selected; the path bar names the disk
    const nh = eo(pr(t, 10650, 420));
    put(E.nw, 'height', `${n(24 * nh)}px`); put(E.nwr, 'opacity', n(eo(pr(t, 10710, 320)), 3));
    flag(E.nwr, 'fresh', t >= 10650 && t < 10970); flag(E.nwr, 'sel', t >= 10970);
    E.fl.forEach(el => put(el, 'opacity', n(eo(pr(t, 11010, 300)), 3)));
    put(E.hd, 'backgroundColor', `rgba(0, 122, 255, ${n(.14 * eo(pr(t, 12100, 450)), 3)})`);

    // Menu bar: the app in front, and the Hushpen glyph while Hushpen runs
    const front = t >= 6350 && t < 10000 ? 1 : t >= 10000 && t < 13400 ? 2 : 0;
    E.mg.forEach((el, i) => put(el, 'opacity', i === front ? '1' : '0'));
    put(E.mhp, 'opacity', n(eo(pr(t, 6500, 400)) * (1 - pr(t, 13400, 300)), 3));
  };

  const pointer = t => {
    let i = 1;
    while (i < PK.length - 1 && t > PK[i][0]) i++;
    const a = PK[i - 1], b = PK[i], p = glide(cl((t - a[0]) / (b[0] - a[0] || 1)));
    const A = L[a[1]].tf, B = L[b[1]].tf;
    const x = lerp(A[0] + a[2] * A[2], B[0] + b[2] * B[2], p), y = lerp(A[1] + a[3] * A[2], B[1] + b[3] * B[2], p), sc = lerp(A[2], B[2], p);
    let s = 1;
    PRESS.forEach(c => { s = Math.min(s, 1 - .14 * Math.sin(Math.PI * pr(t, c - 70, 220))); });
    // The tip stays on its point while the pointer presses
    put(cur, 'transform', `translate(${n(x - 2 * sc * s)}px, ${n(y - 2 * sc * s)}px) scale(${n(sc * s, 4)})`);
  };

  const frame = t => {
    const cx = CAM[0](t), cy = CAM[1](t), z = Math.exp(CAM[2](t));
    for (const k in L) {
      const w = L[k], win = WINS.includes(k), [o, s, dy, b, ia] = win ? state(k, t) : [1, 1, 0, 0, false];
      const zd = z ** w.d, fx = CX + (cx - CX) * w.d, fy = CY + (cy - CY) * w.d, sc = w.k * s * zd;
      const ox = w.x + w.w * w.k * (1 - s) / 2, oy = w.y + w.h * w.k * (1 - s) / 2 + dy;
      w.tf = [CX + (ox - fx) * zd, CY + (oy - fy) * zd, sc];
      if (!win) { put(w.el, 'transform', `translate(${n(w.tf[0], 3)}px, ${n(w.tf[1], 3)}px) scale(${n(sc, 5)})`); continue; }
      put(w.el, 'visibility', o > .002 ? 'visible' : 'hidden');
      put(w.el, 'opacity', o >= .999 ? '1' : n(o, 3));
      flag(w.el, 'ia', ia);
      if (o <= .002) continue;
      put(w.el, 'transform', `translate(${n(w.tf[0], 3)}px, ${n(w.tf[1], 3)}px) scale(${n(sc, 5)})`);
      // A filter blurs in the window's own pixels, so the radius is divided by its scale
      put(w.el, 'filter', b > .05 ? `blur(${n(b / sc)}px)` : 'none');
    }
    pointer(t);
    scene(t);
  };

  let t = calm ? STILL : 0;
  measure();
  frame(t);
  window.HushpenReleases.subscribe(({ stable }) => {
    const a = stable.assets['macos-dmg'];
    if (a) { bytes = a.size; q('[data-wf-file]').textContent = a.name; }
    host.querySelectorAll('[data-wf-tag]').forEach(el => { el.textContent = stable.tag; });
    host.querySelectorAll('[data-wf-title]').forEach(el => { el.textContent = `Hushpen ${stable.version}`; });
    q('[data-wf-published]').textContent = `released ${stable.publishedAt.slice(0, 10)}`;
    q('[data-wf-count]').textContent = Object.keys(stable.assets).length;
    const note = q('.wg-nt');
    note.textContent = stable.highlight || ''; note.hidden = !stable.highlight;
    host.querySelectorAll('[data-wf-asset]').forEach(row => {
      const file = stable.assets[row.dataset.wfAsset];
      row.hidden = !file;
      if (!file) return;
      row.querySelector('b').textContent = file.name;
      row.querySelector('em').textContent = window.HushpenReleases.size(file.size);
      row.querySelector('time').textContent = stable.publishedAt.slice(0, 10);
    });
    measure(); frame(t);
  });
  document.fonts?.ready.then(() => { measure(); frame(t); });
  if (calm) return;
  let last = 0, raf = 0;
  const tick = now => {
    t = (t + (last ? Math.min(50, now - last) : 0)) % T;
    last = now;
    frame(t);
    raf = requestAnimationFrame(tick);
  };
  new IntersectionObserver(([e]) => {
    if (e.isIntersecting && !raf) { last = 0; raf = requestAnimationFrame(tick); }
    else if (!e.isIntersecting && raf) { cancelAnimationFrame(raf); raf = 0; }
  }, { threshold: 0.2 }).observe(host);
})();
