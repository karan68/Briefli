'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChevronDown, ChevronUp } from 'lucide-react';
import { toast } from 'sonner';
import { ConfirmationModal } from '@/components/ConfirmationModel/confirmation-modal';
import { createUniqueTemplateId, moveArrayItem } from '@/lib/summary-template';

interface TemplateInfo {
  id: string;
  name: string;
  description: string;
  source: string;
  editable: boolean;
  deletable: boolean;
}

type SectionFormat = 'paragraph' | 'list' | 'string';

interface TemplateSection {
  title: string;
  instruction: string;
  format: SectionFormat;
  item_format?: string;
  example_item_format?: string;
}

interface TemplateContent {
  name: string;
  description: string;
  sections: TemplateSection[];
}

interface EditorState {
  // Non-null when editing an existing id (a custom template or a built-in override).
  // Null for a brand-new / duplicated template — the id is derived from the name on save.
  existingId: string | null;
  name: string;
  description: string;
  sections: TemplateSection[];
}

const FORMAT_OPTIONS: { value: SectionFormat; label: string }[] = [
  { value: 'paragraph', label: 'Paragraph' },
  { value: 'list', label: 'List' },
  { value: 'string', label: 'Single line' },
];

const MAX_TEMPLATE_NAME_CHARS = 120;
const MAX_TEMPLATE_DESCRIPTION_CHARS = 500;
const MAX_TEMPLATE_SECTIONS = 30;
const MAX_SECTION_TITLE_CHARS = 120;
const MAX_SECTION_INSTRUCTION_CHARS = 2_000;
const MAX_ITEM_FORMAT_CHARS = 500;

const SOURCE_BADGE: Record<string, { label: string; className: string }> = {
  custom: { label: 'Custom', className: 'bg-blue-100 text-blue-700' },
  bundled: { label: 'Built-in', className: 'bg-gray-100 text-gray-600' },
  built_in: { label: 'Built-in', className: 'bg-gray-100 text-gray-600' },
};

function emptySection(): TemplateSection {
  return { title: '', instruction: '', format: 'paragraph' };
}

function editableSections(sections: TemplateSection[]): TemplateSection[] {
  return sections.map(({ example_item_format, ...section }) => ({
    ...section,
    item_format: section.item_format ?? example_item_format,
  }));
}

function errorMessage(err: unknown, fallback: string): string {
  return typeof err === 'string' ? err : fallback;
}

function serializeEditorToJson(e: EditorState): string {
  const content = {
    name: e.name,
    description: e.description,
    sections: e.sections.map((s) => {
      const section: Record<string, unknown> = {
        title: s.title,
        instruction: s.instruction,
        format: s.format,
      };
      if (s.item_format && s.item_format.trim()) section.item_format = s.item_format;
      if (s.example_item_format && s.example_item_format.trim()) {
        section.example_item_format = s.example_item_format;
      }
      return section;
    }),
  };
  return JSON.stringify(content, null, 2);
}

function parseJsonToEditor(text: string, existingId: string | null): EditorState {
  const parsed = JSON.parse(text) as Partial<TemplateContent>;
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('Template must be a JSON object.');
  }
  const rawSections = Array.isArray(parsed.sections) ? (parsed.sections as TemplateSection[]) : [];
  return {
    existingId,
    name: typeof parsed.name === 'string' ? parsed.name : '',
    description: typeof parsed.description === 'string' ? parsed.description : '',
    sections: rawSections.length ? editableSections(rawSections) : [emptySection()],
  };
}

export function SummaryTemplateManager() {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [initialEditor, setInitialEditor] = useState<EditorState | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [saving, setSaving] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<TemplateInfo | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [jsonMode, setJsonMode] = useState(false);
  const [jsonText, setJsonText] = useState('');

  const loadTemplates = useCallback(async () => {
    try {
      setLoading(true);
      const list = await invoke<TemplateInfo[]>('api_list_templates');
      setTemplates(list);
    } catch (err) {
      console.error('Failed to load templates:', err);
      toast.error('Failed to load templates');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadTemplates();
  }, [loadTemplates]);

  const openEditor = (next: EditorState) => {
    setEditor(next);
    setInitialEditor(next);
    setJsonMode(false);
    setJsonText('');
  };

  const closeEditor = () => {
    setEditor(null);
    setInitialEditor(null);
    setConfirmDiscard(false);
    setJsonMode(false);
    setJsonText('');
  };

  const openNew = () => {
    openEditor({ existingId: null, name: '', description: '', sections: [emptySection()] });
  };

  const openImport = () => {
    const skeleton: EditorState = {
      existingId: null,
      name: '',
      description: '',
      sections: [emptySection()],
    };
    setEditor(skeleton);
    setInitialEditor(skeleton);
    setJsonText(serializeEditorToJson(skeleton));
    setJsonMode(true);
  };

  const openEdit = async (t: TemplateInfo) => {
    try {
      const content = await invoke<TemplateContent>('api_get_template_content', { templateId: t.id });
      openEditor({
        existingId: t.id,
        name: content.name,
        description: content.description,
        sections: content.sections.length ? editableSections(content.sections) : [emptySection()],
      });
    } catch (err) {
      console.error('Failed to open template:', err);
      toast.error(errorMessage(err, 'Failed to open template'));
    }
  };

  const openDuplicate = async (t: TemplateInfo) => {
    try {
      const content = await invoke<TemplateContent>('api_get_template_content', { templateId: t.id });
      openEditor({
        existingId: null,
        name: `${content.name} (copy)`,
        description: content.description,
        sections: content.sections.length ? editableSections(content.sections) : [emptySection()],
      });
    } catch (err) {
      console.error('Failed to duplicate template:', err);
      toast.error(errorMessage(err, 'Failed to duplicate template'));
    }
  };

  const enterJsonMode = () => {
    if (!editor) return;
    setJsonText(serializeEditorToJson(editor));
    setJsonMode(true);
  };

  const exitJsonMode = () => {
    if (!editor) return;
    try {
      setEditor(parseJsonToEditor(jsonText, editor.existingId));
      setJsonMode(false);
    } catch (err) {
      toast.error(`Invalid JSON: ${(err as Error).message}`);
    }
  };

  const validateJson = async () => {
    try {
      const name = await invoke<string>('api_validate_template', { templateJson: jsonText });
      toast.success(`Valid template: "${name}"`);
    } catch (err) {
      toast.error(errorMessage(err, 'Template is invalid'));
    }
  };

  const copyTemplateJson = async (t: TemplateInfo) => {
    try {
      const content = await invoke<TemplateContent>('api_get_template_content', { templateId: t.id });
      await navigator.clipboard.writeText(JSON.stringify(content, null, 2));
      toast.success('Template JSON copied to clipboard');
    } catch (err) {
      console.error('Failed to copy template:', err);
      toast.error(errorMessage(err, 'Failed to copy template'));
    }
  };

  const patchSection = (index: number, patch: Partial<TemplateSection>) => {
    setEditor((e) =>
      e ? { ...e, sections: e.sections.map((s, i) => (i === index ? { ...s, ...patch } : s)) } : e
    );
  };

  const addSection = () => setEditor((e) => (e ? { ...e, sections: [...e.sections, emptySection()] } : e));

  const removeSection = (index: number) =>
    setEditor((e) => (e ? { ...e, sections: e.sections.filter((_, i) => i !== index) } : e));

  const moveSection = (from: number, to: number) =>
    setEditor((e) => (e ? { ...e, sections: moveArrayItem(e.sections, from, to) } : e));

  const proposedId = editor?.existingId
    ?? createUniqueTemplateId(editor?.name ?? '', templates.map((template) => template.id));

  const validate = (e: EditorState): string | null => {
    if (!e.name.trim()) return 'Name is required.';
    if (!e.description.trim()) return 'Description is required.';
    if (e.sections.length === 0) return 'Add at least one section.';
    const titles = new Set<string>();
    for (const s of e.sections) {
      if (!s.title.trim()) return 'Every section needs a title.';
      if (!s.instruction.trim()) return `Section "${s.title.trim() || '…'}" needs an instruction.`;
      const normalizedTitle = s.title.trim().toLowerCase();
      if (titles.has(normalizedTitle)) return `Section title "${s.title.trim()}" is duplicated.`;
      titles.add(normalizedTitle);
    }
    if (e.existingId === null && !proposedId) return 'Name must contain letters or numbers.';
    return null;
  };

  const save = async () => {
    if (!editor) return;

    if (jsonMode) {
      let parsed: unknown;
      try {
        parsed = JSON.parse(jsonText);
      } catch (err) {
        toast.error(`Invalid JSON: ${(err as Error).message}`);
        return;
      }
      const parsedName =
        parsed && typeof parsed === 'object' &&
        typeof (parsed as { name?: unknown }).name === 'string'
          ? (parsed as { name: string }).name.trim()
          : '';
      const jsonId =
        editor.existingId ?? createUniqueTemplateId(parsedName, templates.map((t) => t.id));
      if (editor.existingId === null && !jsonId) {
        toast.error('Template "name" must contain letters or numbers.');
        return;
      }
      try {
        setSaving(true);
        await invoke('api_save_template', { templateId: jsonId, templateJson: jsonText });
        toast.success(`Template "${parsedName || jsonId}" saved`);
        closeEditor();
        await loadTemplates();
      } catch (err) {
        console.error('Failed to save template:', err);
        toast.error(errorMessage(err, 'Failed to save template'));
      } finally {
        setSaving(false);
      }
      return;
    }

    const problem = validate(editor);
    if (problem) {
      toast.error(problem);
      return;
    }

    const id = editor.existingId ?? proposedId;
    const content: TemplateContent = {
      name: editor.name.trim(),
      description: editor.description.trim(),
      sections: editor.sections.map((s) => {
        const section: TemplateSection = {
          title: s.title.trim(),
          instruction: s.instruction.trim(),
          format: s.format,
        };
        if (s.format === 'list' && s.item_format?.trim()) {
          section.item_format = s.item_format.trim();
        }
        return section;
      }),
    };

    try {
      setSaving(true);
      await invoke('api_save_template', { templateId: id, templateJson: JSON.stringify(content) });
      toast.success(`Template "${content.name}" saved`);
      closeEditor();
      await loadTemplates();
    } catch (err) {
      console.error('Failed to save template:', err);
      toast.error(errorMessage(err, 'Failed to save template'));
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = async () => {
    if (!pendingDelete) return;
    try {
      setDeleting(true);
      await invoke('api_delete_template', { templateId: pendingDelete.id });
      toast.success(`Template "${pendingDelete.name}" deleted`);
      await loadTemplates();
    } catch (err) {
      console.error('Failed to delete template:', err);
      toast.error(errorMessage(err, 'Failed to delete template'));
    } finally {
      setDeleting(false);
      setPendingDelete(null);
    }
  };

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
      <div className="flex items-start justify-between gap-4 mb-1">
        <div>
          <h3 className="text-lg font-semibold text-gray-900">Summary Templates</h3>
          <p className="text-sm text-gray-600">
            Customize how meeting summaries are structured. A template&apos;s sections define what the
            AI extracts and how each part is written.
          </p>
        </div>
        {!editor && (
          <div className="flex shrink-0 items-center gap-2">
            <button
              onClick={openImport}
              className="border border-gray-300 text-gray-700 px-3 py-1.5 rounded-md text-sm font-medium hover:bg-gray-50 transition-colors"
            >
              Import JSON
            </button>
            <button
              onClick={openNew}
              className="bg-blue-600 text-white px-3 py-1.5 rounded-md text-sm font-medium hover:bg-blue-700 transition-colors"
            >
              New template
            </button>
          </div>
        )}
      </div>

      {editor ? (
        <TemplateEditor
          editor={editor}
          proposedId={proposedId}
          saving={saving}
          jsonMode={jsonMode}
          jsonText={jsonText}
          onEnterJsonMode={enterJsonMode}
          onExitJsonMode={exitJsonMode}
          onChangeJson={setJsonText}
          onValidateJson={validateJson}
          onChangeName={(name) => setEditor((e) => (e ? { ...e, name } : e))}
          onChangeDescription={(description) => setEditor((e) => (e ? { ...e, description } : e))}
          onPatchSection={patchSection}
          onAddSection={addSection}
          onRemoveSection={removeSection}
          onMoveSection={moveSection}
          onCancel={() => {
            const baseline = initialEditor ? serializeEditorToJson(initialEditor) : '';
            const current = jsonMode ? jsonText : editor ? serializeEditorToJson(editor) : '';
            if (baseline !== current) {
              setConfirmDiscard(true);
            } else {
              closeEditor();
            }
          }}
          onSave={save}
        />
      ) : loading ? (
        <p className="text-sm text-gray-500 py-4">Loading templates…</p>
      ) : (
        <ul className="mt-4 space-y-2">
          {templates.map((t) => {
            const badge = SOURCE_BADGE[t.source] ?? { label: 'Unknown', className: 'bg-gray-100 text-gray-600' };
            return (
              <li
                key={t.id}
                className="flex items-start justify-between gap-4 rounded-lg border border-gray-200 p-3"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-gray-900 truncate">{t.name}</span>
                    <span className={`px-2 py-0.5 rounded-full text-xs ${badge.className}`}>
                      {badge.label}
                    </span>
                  </div>
                  <p className="text-sm text-gray-600 mt-0.5 line-clamp-2">{t.description}</p>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <button
                    onClick={() => openEdit(t)}
                    className="text-gray-600 hover:text-blue-700 px-2 py-1 rounded text-sm transition-colors"
                  >
                    Edit
                  </button>
                  <button
                    onClick={() => openDuplicate(t)}
                    className="text-gray-600 hover:text-blue-700 px-2 py-1 rounded text-sm transition-colors"
                  >
                    Duplicate
                  </button>
                  <button
                    onClick={() => copyTemplateJson(t)}
                    className="text-gray-600 hover:text-blue-700 px-2 py-1 rounded text-sm transition-colors"
                  >
                    Copy JSON
                  </button>
                  {t.deletable && (
                    <button
                      onClick={() => setPendingDelete(t)}
                      className="text-gray-400 hover:text-red-600 px-2 py-1 rounded text-sm transition-colors"
                    >
                      Delete
                    </button>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}

      <ConfirmationModal
        isOpen={pendingDelete !== null}
        title="Delete template"
        text={
          pendingDelete
            ? `Delete the custom template "${pendingDelete.name}"? If it overrides a built-in template, the built-in version will be restored.`
            : ''
        }
        confirmLabel="Delete"
        isConfirming={deleting}
        onConfirm={confirmDelete}
        onCancel={() => {
          if (!deleting) setPendingDelete(null);
        }}
      />
      <ConfirmationModal
        isOpen={confirmDiscard}
        title="Discard template changes?"
        text="Your unsaved changes will be lost."
        confirmLabel="Discard"
        onConfirm={closeEditor}
        onCancel={() => setConfirmDiscard(false)}
      />
    </div>
  );
}

interface TemplateEditorProps {
  editor: EditorState;
  proposedId: string;
  saving: boolean;
  jsonMode: boolean;
  jsonText: string;
  onEnterJsonMode: () => void;
  onExitJsonMode: () => void;
  onChangeJson: (text: string) => void;
  onValidateJson: () => void;
  onChangeName: (name: string) => void;
  onChangeDescription: (description: string) => void;
  onPatchSection: (index: number, patch: Partial<TemplateSection>) => void;
  onAddSection: () => void;
  onRemoveSection: (index: number) => void;
  onMoveSection: (from: number, to: number) => void;
  onCancel: () => void;
  onSave: () => void;
}

function TemplateEditor({
  editor,
  proposedId,
  saving,
  jsonMode,
  jsonText,
  onEnterJsonMode,
  onExitJsonMode,
  onChangeJson,
  onValidateJson,
  onChangeName,
  onChangeDescription,
  onPatchSection,
  onAddSection,
  onRemoveSection,
  onMoveSection,
  onCancel,
  onSave,
}: TemplateEditorProps) {
  const isNew = editor.existingId === null;
  const savedAs = isNew ? proposedId : editor.existingId;

  return (
    <div className="mt-4 space-y-4">
      <div className="flex items-center justify-between">
        <div className="inline-flex overflow-hidden rounded-md border border-gray-300 text-sm">
          <button
            type="button"
            onClick={() => { if (jsonMode) onExitJsonMode(); }}
            className={`px-3 py-1.5 ${jsonMode ? 'bg-white text-gray-700 hover:bg-gray-50' : 'bg-blue-600 text-white'}`}
          >
            Form
          </button>
          <button
            type="button"
            onClick={() => { if (!jsonMode) onEnterJsonMode(); }}
            className={`border-l border-gray-300 px-3 py-1.5 ${jsonMode ? 'bg-blue-600 text-white' : 'bg-white text-gray-700 hover:bg-gray-50'}`}
          >
            JSON
          </button>
        </div>
        {jsonMode && (
          <button
            type="button"
            onClick={onValidateJson}
            className="text-sm font-medium text-blue-600 hover:text-blue-700"
          >
            Validate
          </button>
        )}
      </div>

      {!isNew && (
        <p className="text-xs text-gray-500">
          Editing saves a custom copy of this template. You can revert by deleting the custom copy.
        </p>
      )}

      {jsonMode ? (
        <div className="space-y-2">
          <textarea
            value={jsonText}
            onChange={(e) => onChangeJson(e.target.value)}
            spellCheck={false}
            rows={18}
            className="w-full rounded-md border border-gray-300 px-3 py-2 font-mono text-xs focus:border-blue-500 focus:outline-none resize-y"
          />
          <p className="text-xs text-gray-400">
            Schema: <code>{'{ name, description, sections: [{ title, instruction, format }] }'}</code>.
            Each section&apos;s format must be paragraph, list, or string; list sections may add an
            optional item_format.
          </p>
        </div>
      ) : (
      <>
      <div>
        <label className="block text-sm font-medium text-gray-700 mb-1">Name</label>
        <input
          type="text"
          value={editor.name}
          maxLength={MAX_TEMPLATE_NAME_CHARS}
          onChange={(e) => onChangeName(e.target.value)}
          placeholder="e.g. Client call"
          className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none"
        />
        {isNew && savedAs && (
          <p className="text-xs text-gray-400 mt-1">Saved as: {savedAs}</p>
        )}
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700 mb-1">Description</label>
        <input
          type="text"
          value={editor.description}
          maxLength={MAX_TEMPLATE_DESCRIPTION_CHARS}
          onChange={(e) => onChangeDescription(e.target.value)}
          placeholder="Short summary of when to use this template"
          className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none"
        />
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <label className="block text-sm font-medium text-gray-700">Sections</label>
          <button
            onClick={onAddSection}
            disabled={editor.sections.length >= MAX_TEMPLATE_SECTIONS}
            title={editor.sections.length >= MAX_TEMPLATE_SECTIONS ? `Maximum ${MAX_TEMPLATE_SECTIONS} sections` : undefined}
            className="text-sm text-blue-600 hover:text-blue-700 font-medium disabled:text-gray-400"
          >
            + Add section
          </button>
        </div>

        {editor.sections.map((section, index) => (
          <div key={index} className="rounded-lg border border-gray-200 p-3 space-y-3">
            <div className="flex flex-wrap items-center gap-3">
              <input
                type="text"
                value={section.title}
                maxLength={MAX_SECTION_TITLE_CHARS}
                onChange={(e) => onPatchSection(index, { title: e.target.value })}
                placeholder="Section title (e.g. Action Items)"
                className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none"
              />
              <select
                value={section.format}
                onChange={(e) => onPatchSection(index, { format: e.target.value as SectionFormat })}
                className="rounded-md border border-gray-300 px-2 py-2 text-sm focus:border-blue-500 focus:outline-none"
              >
                {FORMAT_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
              <button
                onClick={() => onMoveSection(index, index - 1)}
                disabled={index === 0}
                title="Move section up"
                aria-label={`Move ${section.title || `section ${index + 1}`} up`}
                className="text-gray-500 hover:text-gray-900 p-1 rounded transition-colors disabled:opacity-30 disabled:hover:text-gray-500"
              >
                <ChevronUp className="h-4 w-4" />
              </button>
              <button
                onClick={() => onMoveSection(index, index + 1)}
                disabled={index === editor.sections.length - 1}
                title="Move section down"
                aria-label={`Move ${section.title || `section ${index + 1}`} down`}
                className="text-gray-500 hover:text-gray-900 p-1 rounded transition-colors disabled:opacity-30 disabled:hover:text-gray-500"
              >
                <ChevronDown className="h-4 w-4" />
              </button>
              <button
                onClick={() => onRemoveSection(index)}
                disabled={editor.sections.length <= 1}
                title={editor.sections.length <= 1 ? 'A template needs at least one section' : 'Remove section'}
                className="text-gray-400 hover:text-red-600 px-2 py-1 rounded text-sm transition-colors disabled:opacity-40 disabled:hover:text-gray-400"
              >
                Remove
              </button>
            </div>

            <textarea
              value={section.instruction}
              maxLength={MAX_SECTION_INSTRUCTION_CHARS}
              onChange={(e) => onPatchSection(index, { instruction: e.target.value })}
              placeholder="Instruction for the AI — what to extract or write for this section"
              rows={2}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none resize-y"
            />

            {section.format === 'list' && (
              <input
                type="text"
                value={section.item_format ?? ''}
                maxLength={MAX_ITEM_FORMAT_CHARS}
                onChange={(e) => onPatchSection(index, { item_format: e.target.value })}
                placeholder="Optional: list item format hint (e.g. - [owner]: [task])"
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none"
              />
            )}
          </div>
        ))}
      </div>
      </>
      )}

      <div className="flex justify-end gap-2 pt-1">
        <button
          onClick={onCancel}
          disabled={saving}
          className="px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded-md transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          onClick={onSave}
          disabled={saving}
          className="px-4 py-2 text-sm bg-blue-600 text-white rounded-md font-medium hover:bg-blue-700 transition-colors disabled:opacity-50"
        >
          {saving ? 'Saving…' : 'Save template'}
        </button>
      </div>
    </div>
  );
}
