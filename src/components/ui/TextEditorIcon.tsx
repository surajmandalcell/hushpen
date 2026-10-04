import type { TextEditorOption } from "../../types/settings";

export function TextEditorIcon({ editor, className }: { editor: TextEditorOption; className: string }) {
  if (!editor.iconDataUrl) {
    return <span aria-hidden="true" data-editor-icon={editor.id} className={className} />;
  }

  return (
    <img
      alt=""
      aria-hidden="true"
      data-editor-icon={editor.id}
      className={`${className} rounded-[3px] object-contain`}
      src={editor.iconDataUrl}
    />
  );
}
