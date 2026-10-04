import { ArrowUpRight } from "lucide-react";
import { AppLogoMark } from "../../components/icons";
import { iconTileClass, panelDividerClass, panelSurfaceClass } from "../../components/ui/classes";
import { ViewFrame } from "../../components/ui/ViewFrame";
import type { ViewProps } from "../../types/view";
import { aboutActionLinks, buildAboutFactRows } from "./aboutContent";

export function AboutView({ appInfo, runtimeInfo }: ViewProps) {
  const facts = buildAboutFactRows(appInfo.version, runtimeInfo?.dataDir);

  return (
    <ViewFrame title="About ASR Pro">
      <section aria-label="About product summary" className={panelSurfaceClass}>
        <div className="flex flex-col gap-4 p-5 sm:flex-row sm:items-start">
          <div
            data-brand-icon-surface="ink-slate"
            className="grid size-[72px] shrink-0 place-items-center rounded-[16px] text-[#eef4f5] shadow-[inset_0_1px_0_rgba(255,255,255,0.2),inset_0_-16px_24px_rgba(0,0,0,0.45)]"
            style={{ background: "linear-gradient(145deg, #20272d, #10171d 50%, #04070a)" }}
          >
            <AppLogoMark className="size-16 opacity-[0.88]" title="ASR Pro" />
          </div>
          <div className="min-w-0">
            <h3 className="text-[24px] font-semibold leading-7 tracking-normal text-[#f4f4f4]">{appInfo.name}</h3>
            <p className="selectable-text mt-1 text-[12px] font-semibold text-[#a8a8a8]">Version {appInfo.version}</p>
            <p className="selectable-text mt-4 max-w-[420px] text-[13px] leading-5 text-[#cfcfcf]">
              A quiet desktop workspace for private dictation, file transcription, and local speech model testing.
            </p>
          </div>
        </div>

        <dl aria-label="Product facts" className={`border-t ${panelDividerClass}`}>
          {facts.map((fact) => (
            <div key={fact.label} className={`grid gap-1 border-t ${panelDividerClass} px-5 py-3 first:border-t-0 sm:grid-cols-[120px_minmax(0,1fr)] sm:gap-4`}>
              <dt className="text-[11px] font-semibold uppercase leading-5 text-[#8e8e8e]">{fact.label}</dt>
              <dd className="selectable-text text-[13px] font-semibold leading-5 text-[#e4e4e4]">{fact.value}</dd>
            </div>
          ))}
        </dl>

        <div aria-label="GitHub links" className={`grid border-t ${panelDividerClass} sm:grid-cols-2`}>
          {aboutActionLinks.map((link) => {
            const Icon = link.icon;

            return (
              <a
                key={link.label}
                href={link.href}
                target="_blank"
                rel="noreferrer"
                className={`group/link flex min-w-0 items-center gap-3 border-t ${panelDividerClass} px-5 py-3 text-left no-underline transition-colors first:border-t-0 hover:bg-white/[0.045] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#9bcfff]/70 sm:border-l sm:border-t-0 sm:first:border-l-0`}
                aria-label={`${link.label}: ${link.detail}`}
              >
                <span className={iconTileClass}>
                  <Icon className="size-3.5" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[13px] font-semibold leading-5 text-[#eeeeee]">{link.label}</span>
                  <span className="block truncate text-[12px] font-medium leading-5 text-[#aaa]">{link.detail}</span>
                </span>
                <ArrowUpRight className="size-3.5 shrink-0 text-[#9f9f9f] transition-colors group-hover/link:text-[#eeeeee]" />
              </a>
            );
          })}
        </div>
      </section>
    </ViewFrame>
  );
}
