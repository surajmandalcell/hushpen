import { useEffect, useRef, useState } from "react";
import { Check } from "lucide-react";
import { DropdownOptionButton, DropdownSurface } from "../../components/ui/Dropdown";
import { PanelControlButton } from "../../components/ui/PanelControlButton";
import { TextEditorIcon } from "../../components/ui/TextEditorIcon";
import type { TextEditorOption } from "../../types/settings";

interface TextEditorSelectorProps {
  options: TextEditorOption[];
  selectedEditorId: string;
  selectedLabel: string;
  onSelect: (editorId: string) => void;
}

export function TextEditorSelector({ options, selectedEditorId, selectedLabel, onSelect }: TextEditorSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const listboxId = useRef(`text-editor-options-${Math.random().toString(36).slice(2)}`);
  const selectedEditor = options.find((editor) => editor.id === selectedEditorId) ?? {
    id: selectedEditorId,
    label: selectedLabel,
    detail: "",
  };

  useEffect(() => {
    if (!isOpen) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setIsOpen(false);
    };

    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  const handleSelect = (editorId: string) => {
    onSelect(editorId);
    setIsOpen(false);
  };

  return (
    <div ref={rootRef} className="relative min-w-[180px]">
      <PanelControlButton
        type="button"
        aria-controls={isOpen ? listboxId.current : undefined}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label="Text editor selector"
        className="w-full min-w-0 justify-start px-2 py-1.5 text-left"
        onClick={() => setIsOpen((current) => !current)}
      >
        <TextEditorIcon editor={selectedEditor} className="size-3 shrink-0" />
        <span className="min-w-0 flex-1 truncate">{selectedLabel}</span>
      </PanelControlButton>

      {isOpen ? (
        <DropdownSurface
          id={listboxId.current}
          ariaLabel="Text editor options"
          alignClassName="right-0 top-full mt-1 w-[260px] max-w-[calc(100vw-1rem)]"
        >
          {options.map((editor) => {
            const selected = editor.id === selectedEditorId;

            return (
              <DropdownOptionButton
                key={editor.id}
                selected={selected}
                aria-label={editor.label}
                onClick={() => handleSelect(editor.id)}
              >
                <TextEditorIcon editor={editor} className="mt-0.5 size-3 shrink-0" />
                <span className="min-w-0 flex-1 whitespace-normal break-words">{editor.label}</span>
                {selected ? <Check className="mt-0.5 size-3 shrink-0 text-[#9bcfff]" /> : null}
              </DropdownOptionButton>
            );
          })}
        </DropdownSurface>
      ) : null}
    </div>
  );
}
