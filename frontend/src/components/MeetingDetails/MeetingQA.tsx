"use client";

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Sparkles, Loader2, Send } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@/components/ui/sheet';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import Analytics from '@/lib/analytics';

interface AnswerSource {
  number: number;
  segmentId: string;
  timestamp: string;
  text: string;
}

interface MeetingAnswer {
  answer: string;
  sources: AnswerSource[];
  provider: string;
  model: string;
  excerptsUsed: number;
}

interface MeetingQAProps {
  meetingId: string;
  hasTranscripts: boolean;
}

/**
 * "Ask about this meeting" — a slide-over that answers questions using only the
 * current meeting's transcript, via the local `api_ask_meeting_question` command.
 */
export function MeetingQA({ meetingId, hasTranscripts }: MeetingQAProps) {
  const [open, setOpen] = useState(false);
  const [question, setQuestion] = useState('');
  const [isAsking, setIsAsking] = useState(false);
  const [result, setResult] = useState<MeetingAnswer | null>(null);

  const ask = async () => {
    const trimmed = question.trim();
    if (!trimmed || isAsking) return;

    setIsAsking(true);
    setResult(null);
    try {
      const answer = await invoke<MeetingAnswer>('api_ask_meeting_question', {
        meetingId,
        question: trimmed,
      });
      setResult(answer);
      void Analytics.trackButtonClick('ask_meeting_question', 'meeting_details');
    } catch (error) {
      const message = typeof error === 'string' ? error : 'Failed to answer the question.';
      toast.error(message);
    } finally {
      setIsAsking(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      ask();
    }
  };

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button
          className="gap-2 rounded-full shadow-lg"
          disabled={!hasTranscripts}
          title={
            hasTranscripts
              ? 'Ask a question about this meeting'
              : 'No transcript to ask about yet'
          }
        >
          <Sparkles className="h-4 w-4" />
          Ask this meeting
        </Button>
      </SheetTrigger>

      <SheetContent side="right" className="flex w-full flex-col gap-4 sm:max-w-md">
        <SheetHeader>
          <SheetTitle>Ask about this meeting</SheetTitle>
          <SheetDescription>
            Answers come only from this meeting&apos;s transcript, generated locally by your
            configured model.
          </SheetDescription>
        </SheetHeader>

        <div className="flex flex-col gap-2">
          <Textarea
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="e.g. What did we decide about the budget?"
            rows={3}
            disabled={isAsking}
          />
          <div className="flex items-center justify-between">
            <span className="text-xs text-muted-foreground">Ctrl/⌘ + Enter to ask</span>
            <Button onClick={ask} disabled={isAsking || !question.trim()} size="sm" className="gap-2">
              {isAsking ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
              {isAsking ? 'Thinking…' : 'Ask'}
            </Button>
          </div>
        </div>

        {result && (
          <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto">
            <div className="prose prose-sm max-w-none text-sm leading-relaxed text-foreground prose-p:my-2 prose-strong:text-foreground prose-headings:text-foreground">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{result.answer}</ReactMarkdown>
            </div>

            {result.sources.length > 0 && (
              <div className="flex flex-col gap-2">
                <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  Sources
                </div>
                {result.sources.map((source) => (
                  <div
                    key={source.number}
                    className="rounded-md border border-briefli-line bg-briefli-paper/50 p-2 text-xs text-muted-foreground"
                  >
                    <span className="mr-1 font-semibold text-foreground">[{source.number}]</span>
                    {source.timestamp && <span className="mr-1">({source.timestamp})</span>}
                    {source.text}
                  </div>
                ))}
              </div>
            )}

            <div className="text-[10px] text-muted-foreground">
              Answered by {result.provider} · {result.model} · {result.excerptsUsed} excerpt
              {result.excerptsUsed === 1 ? '' : 's'}
            </div>
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}
