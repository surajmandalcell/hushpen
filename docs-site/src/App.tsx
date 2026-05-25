import { useEffect, useMemo, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import {
  ArrowRight,
  ArrowUpRight,
  ChevronLeft,
  ChevronRight,
  CircleArrowOutUpRight,
  Download,
  FolderOpen,
  Github,
  History,
  Mic2,
  Settings2,
  SlidersHorizontal,
} from "lucide-react";

type Slide = {
  label: string;
  title: string;
  text: string;
  image: string;
};

type Step = {
  label: string;
  title: string;
  text: string;
  image: string;
  accent: string;
  icon: typeof Mic2;
};

type GalleryShot = {
  image: string;
  title: string;
  text: string;
  className: string;
};

const heroSlides: Slide[] = [
  {
    label: "Home",
    title: "Capture from anywhere.",
    text: "Start recording from the app, the tray, or a keyboard shortcut without leaving the desktop flow.",
    image: "./screenshots/asrpro-home.png",
  },
  {
    label: "History",
    title: "Search every saved clip.",
    text: "Replay, reopen, copy, delete, or reprocess saved transcripts without hunting through local folders.",
    image: "./screenshots/asrpro-history.png",
  },
  {
    label: "Sound",
    title: "Route the right microphone.",
    text: "Refresh devices, keep the correct input selected, and avoid the usual desktop audio guesswork.",
    image: "./screenshots/asrpro-sound.png",
  },
  {
    label: "Preferences",
    title: "Tune the workspace once.",
    text: "Auto-copy, startup launch, editor targets, and overlay placement stay close to the operating system.",
    image: "./screenshots/asrpro-configuration.png",
  },
  {
    label: "Models",
    title: "Keep Whisper local.",
    text: "Pick the model that fits your machine and stay private without sending recordings to a hosted backend.",
    image: "./screenshots/asrpro-models.png",
  },
  {
    label: "About",
    title: "Know where your data lives.",
    text: "Open storage paths, inspect the install state, and keep portable setups under your control.",
    image: "./screenshots/asrpro-about.png",
  },
];

const workflowSteps: Step[] = [
  {
    label: "01",
    title: "Record fast",
    text: "Home, tray, or shortcut. The app stays ready for short captures instead of forcing a heavyweight workflow.",
    image: "./screenshots/asrpro-home.png",
    accent: "var(--accent-orange)",
    icon: Mic2,
  },
  {
    label: "02",
    title: "Search history",
    text: "Saved clips stay queryable and reusable, with copy, reopen, delete, and reprocess actions built in.",
    image: "./screenshots/asrpro-history.png",
    accent: "var(--accent-purple)",
    icon: History,
  },
  {
    label: "03",
    title: "Control the input",
    text: "Real device selection, refresh, and microphone behavior control live inside the app instead of outside it.",
    image: "./screenshots/asrpro-sound.png",
    accent: "var(--accent-blue)",
    icon: SlidersHorizontal,
  },
  {
    label: "04",
    title: "Shape the workspace",
    text: "Clipboard behavior, editor handoff, startup launch, and overlay placement are configured once and stay out of the way.",
    image: "./screenshots/asrpro-configuration.png",
    accent: "var(--accent-teal)",
    icon: Settings2,
  },
];

const galleryShots: GalleryShot[] = [
  {
    image: "./screenshots/asrpro-models.png",
    title: "Models library",
    text: "Select the right speed and accuracy tradeoff locally.",
    className: "shot-large",
  },
  {
    image: "./screenshots/asrpro-about.png",
    title: "Portable data",
    text: "Inspect storage paths and move installs without losing context.",
    className: "shot-top",
  },
  {
    image: "./screenshots/asrpro-configuration.png",
    title: "Startup behavior",
    text: "Launch logic and editor handoff stay predictable on real desktops.",
    className: "shot-bottom",
  },
];

const supportLinks = [
  {
    title: "Portable data",
    text: "Platform-specific storage layout and move guidance.",
    href: "./portable-data.html",
    icon: FolderOpen,
  },
  {
    title: "Startup launch",
    text: "How launch-at-startup behaves when installs move.",
    href: "./startup.html",
    icon: CircleArrowOutUpRight,
  },
  {
    title: "Source",
    text: "Inspect the repository and release history directly.",
    href: "https://github.com/surajmandalcell/asrpro",
    icon: Github,
  },
];

function normalizeDistance(index: number, active: number, total: number) {
  let diff = index - active;
  if (diff > total / 2) diff -= total;
  if (diff < -total / 2) diff += total;
  return diff;
}

function heroCardStyle(index: number, active: number, total: number, pointerX: number, pointerY: number) {
  const diff = normalizeDistance(index, active, total);

  let opacity = 0;
  let zIndex = 1;
  let transform = "translate3d(0, 36px, -160px) scale(0.78)";

  if (diff === 0) {
    opacity = 1;
    zIndex = 6;
    transform = `translate3d(${pointerX * 14}px, ${pointerY * 14}px, 0) rotateX(${pointerY * -4}deg) rotateY(${pointerX * 6}deg) scale(1)`;
  } else if (diff === 1) {
    opacity = 0.9;
    zIndex = 5;
    transform = "translate3d(116px, 76px, -70px) rotate(6deg) scale(0.88)";
  } else if (diff === 2) {
    opacity = 0.58;
    zIndex = 4;
    transform = "translate3d(192px, 128px, -120px) rotate(10deg) scale(0.74)";
  } else if (diff === -1) {
    opacity = 0.42;
    zIndex = 3;
    transform = "translate3d(-42px, -24px, -80px) rotate(-4deg) scale(0.92)";
  }

  return {
    opacity,
    zIndex,
    transform,
  } satisfies CSSProperties;
}

export default function App() {
  const [headerTint, setHeaderTint] = useState(false);
  const [activeHero, setActiveHero] = useState(0);
  const [activeWorkflow, setActiveWorkflow] = useState(1);
  const [pointer, setPointer] = useState({ x: 0, y: 0 });

  useEffect(() => {
    const onScroll = () => {
      setHeaderTint(window.scrollY > window.innerHeight * 0.4);
    };

    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setActiveHero((current) => (current + 1) % heroSlides.length);
    }, 5200);

    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const nodes = Array.from(document.querySelectorAll<HTMLElement>("[data-reveal]"));
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add("is-visible");
          }
        });
      },
      { threshold: 0.14 },
    );

    nodes.forEach((node) => observer.observe(node));
    return () => observer.disconnect();
  }, []);

  const activeHeroSlide = heroSlides[activeHero];
  const activeWorkflowStep = workflowSteps[activeWorkflow];

  const heroStack = useMemo(
    () =>
      heroSlides.map((slide, index) => (
        <figure
          key={slide.image}
          className="hero-card"
          style={heroCardStyle(index, activeHero, heroSlides.length, pointer.x, pointer.y)}
          aria-hidden={index !== activeHero}
        >
          <img src={slide.image} alt={`${slide.label} screen`} />
        </figure>
      )),
    [activeHero, pointer.x, pointer.y],
  );

  const onHeroPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = ((event.clientX - rect.left) / rect.width - 0.5) * 2;
    const y = ((event.clientY - rect.top) / rect.height - 0.5) * 2;
    setPointer({ x, y });
  };

  return (
    <div className="docs-app">
      <div className="page-noise" aria-hidden="true" />
      <header className={`site-header${headerTint ? " is-tinted" : ""}`}>
        <nav className="nav">
          <a className="brand" href="./index.html" aria-label="ASR Pro home">
            <span>ASR Pro</span>
          </a>

          <div className="nav-links">
            <a className="nav-link optional" href="#workflow">
              Workflow
            </a>
            <a className="nav-link optional" href="#support">
              Support
            </a>
            <a className="nav-link optional" href="https://github.com/surajmandalcell/asrpro/releases" target="_blank" rel="noreferrer">
              Releases
            </a>
            <a className="nav-link primary" href="https://github.com/surajmandalcell/asrpro" target="_blank" rel="noreferrer">
              GitHub
            </a>
          </div>
        </nav>
      </header>

      <main>
        <section className="hero section">
          <div className="hero-copy" data-reveal>
            <p className="hero-kicker">Private desktop speech transcription</p>
            <h1>Desktop dictation, minus the cloud.</h1>
            <p className="hero-body">
              ASR Pro records, transcribes, reprocesses, and keeps history locally with Whisper. It behaves like a desktop tool,
              not a browser tab pretending to be one.
            </p>

            <div className="hero-actions">
              <a className="button button-primary" href="https://github.com/surajmandalcell/asrpro/releases" target="_blank" rel="noreferrer">
                <Download size={18} />
                <span>Download</span>
              </a>
              <a className="button" href="https://github.com/surajmandalcell/asrpro" target="_blank" rel="noreferrer">
                <Github size={18} />
                <span>Source</span>
              </a>
            </div>
          </div>

          <div className="hero-media" data-reveal>
            <div className="hero-stage" onPointerMove={onHeroPointerMove} onPointerLeave={() => setPointer({ x: 0, y: 0 })}>
              <div className="hero-glow hero-glow-one" aria-hidden="true" />
              <div className="hero-glow hero-glow-two" aria-hidden="true" />
              {heroStack}
            </div>

            <div className="hero-meta">
              <div className="hero-meta-copy">
                <span className="eyebrow">{activeHeroSlide.label}</span>
                <h2>{activeHeroSlide.title}</h2>
                <p>{activeHeroSlide.text}</p>
              </div>

              <div className="hero-controls">
                <button type="button" aria-label="Previous screenshot" onClick={() => setActiveHero((current) => (current - 1 + heroSlides.length) % heroSlides.length)}>
                  <ChevronLeft size={18} />
                </button>
                <button type="button" aria-label="Next screenshot" onClick={() => setActiveHero((current) => (current + 1) % heroSlides.length)}>
                  <ChevronRight size={18} />
                </button>
              </div>
            </div>

            <div className="hero-dots" aria-label="Screenshot selector">
              {heroSlides.map((slide, index) => (
                <button
                  key={slide.label}
                  type="button"
                  className={index === activeHero ? "is-active" : ""}
                  aria-label={`Show ${slide.label} screen`}
                  onClick={() => setActiveHero(index)}
                />
              ))}
            </div>
          </div>
        </section>

        <section id="workflow" className="workflow section" data-reveal>
          <div className="workflow-copy">
            <p className="hero-kicker">Workflow</p>
            <h2>Built for short capture loops and constant reuse.</h2>
            <p>
              The product page should prove the workflow, not just describe it. Hover or tap through the core flow and the live surface
              follows along with the actual app screens.
            </p>
          </div>

          <div className="workflow-stage">
            <div className="workflow-preview">
              <span className="workflow-index">{activeWorkflowStep.label}</span>
              <span className="eyebrow">{activeWorkflowStep.title}</span>
              <figure>
                <img src={activeWorkflowStep.image} alt={`${activeWorkflowStep.title} screen`} />
              </figure>
            </div>

            <div className="workflow-rail">
              {workflowSteps.map((step, index) => {
                const Icon = step.icon;
                const active = index === activeWorkflow;
                return (
                  <button
                    key={step.label}
                    type="button"
                    className={`workflow-row${active ? " is-active" : ""}`}
                    style={{ "--row-accent": step.accent } as CSSProperties}
                    onMouseEnter={() => setActiveWorkflow(index)}
                    onFocus={() => setActiveWorkflow(index)}
                    onClick={() => setActiveWorkflow(index)}
                    aria-pressed={active}
                  >
                    <span className="workflow-row-mark">
                      <Icon size={18} />
                    </span>
                    <span className="workflow-row-copy">
                      <strong>{step.title}</strong>
                      <span>{step.text}</span>
                    </span>
                    <ArrowRight size={18} />
                  </button>
                );
              })}
            </div>
          </div>
        </section>

        <section className="gallery section" data-reveal>
          <div className="gallery-copy">
            <p className="hero-kicker">Desktop-native details</p>
            <h2>The quiet parts are the point.</h2>
            <p>
              Local model control, portable data paths, and startup behavior are the details that decide whether a desktop transcription
              tool actually survives daily use.
            </p>
          </div>

          <div className="gallery-grid">
            {galleryShots.map((shot) => (
              <figure key={shot.title} className={`gallery-shot ${shot.className}`}>
                <img src={shot.image} alt={shot.title} />
                <figcaption>
                  <strong>{shot.title}</strong>
                  <span>{shot.text}</span>
                </figcaption>
              </figure>
            ))}
          </div>
        </section>

        <section id="support" className="support section" data-reveal>
          <div className="support-copy">
            <p className="hero-kicker">Support docs</p>
            <h2>Keep the setup predictable.</h2>
            <p>
              The public page should still hand off to the practical docs that matter once the app is installed on a real machine.
            </p>
          </div>

          <div className="support-links">
            {supportLinks.map((link) => {
              const Icon = link.icon;
              return (
                <a key={link.title} className="support-link" href={link.href} target={link.href.startsWith("http") ? "_blank" : undefined} rel="noreferrer">
                  <span className="support-icon">
                    <Icon size={18} />
                  </span>
                  <span className="support-text">
                    <strong>{link.title}</strong>
                    <span>{link.text}</span>
                  </span>
                  <ArrowUpRight size={18} />
                </a>
              );
            })}
          </div>

          <div className="final-cta">
            <div>
              <p className="hero-kicker">Install</p>
              <h2>Keep the audio local.</h2>
            </div>

            <div className="final-actions">
              <a className="button button-primary" href="https://github.com/surajmandalcell/asrpro/releases" target="_blank" rel="noreferrer">
                <Download size={18} />
                <span>Get the latest release</span>
              </a>
              <a className="button" href="https://github.com/surajmandalcell/asrpro" target="_blank" rel="noreferrer">
                <Github size={18} />
                <span>Browse the repo</span>
              </a>
            </div>
          </div>
        </section>
      </main>
    </div>
  );
}
