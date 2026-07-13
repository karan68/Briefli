'use client';

import React, { useState, useEffect } from 'react';
import { Settings2, Mic, Database as DatabaseIcon, SparkleIcon, FlaskConical } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { TranscriptSettings } from '@/components/TranscriptSettings';
import { RecordingSettings } from '@/components/RecordingSettings';
import { PreferenceSettings } from '@/components/PreferenceSettings';
import { SummaryModelSettings } from '@/components/SummaryModelSettings';
import { BetaSettings } from '@/components/BetaSettings';
import { useConfig } from '@/contexts/ConfigContext';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';

// Tabs configuration (constant)
const TABS = [
  { value: 'general', label: 'General', icon: Settings2 },
  { value: 'recording', label: 'Recordings', icon: Mic },
  { value: 'Transcriptionmodels', label: 'Transcription', icon: DatabaseIcon },
  { value: 'summaryModels', label: 'Summary', icon: SparkleIcon },
  { value: 'beta', label: 'Beta', icon: FlaskConical }
] as const;

export default function SettingsPage() {
  const { transcriptModelConfig, setTranscriptModelConfig } = useConfig();

  const [activeTab, setActiveTab] = useState('general');

  // Load saved transcript configuration on mount
  useEffect(() => {
    const loadTranscriptConfig = async () => {
      try {
        const config = await invoke('api_get_transcript_config') as any;
        if (config) {
          console.log('Loaded saved transcript config:', config);
          setTranscriptModelConfig({
            provider: config.provider || 'localWhisper',
            model: config.model || 'large-v3',
            apiKey: config.apiKey || null
          });
        }
      } catch (error) {
        console.error('Failed to load transcript config:', error);
      }
    };
    loadTranscriptConfig();
  }, [setTranscriptModelConfig]);

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-briefli-paper">
      <header className="border-b border-briefli-line px-8 py-6">
        <div className="mx-auto max-w-6xl">
          <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-briefli-muted">Application preferences</p>
          <h1 className="font-brand text-3xl font-semibold text-briefli-ink">Settings</h1>
        </div>
      </header>

      <div className="min-h-0 flex-1">
        <div className="mx-auto h-full max-w-6xl px-8 py-6">
          <Tabs value={activeTab} onValueChange={setActiveTab} className="grid h-full grid-cols-[170px_minmax(0,1fr)] gap-8">
            <TabsList className="flex h-fit flex-col items-stretch gap-1 rounded-none bg-transparent p-0">
              {TABS.map((tab) => {
                const Icon = tab.icon;
                return (
                  <TabsTrigger
                    key={tab.value}
                    value={tab.value}
                    className="flex h-10 justify-start gap-3 rounded px-3 text-sm text-briefli-muted shadow-none hover:bg-briefli-sidebar hover:text-briefli-ink data-[state=active]:bg-[#202621] data-[state=active]:text-white data-[state=active]:shadow-none"
                  >
                    <Icon className="w-4 h-4" />
                    {tab.label}
                  </TabsTrigger>
                );
              })}
            </TabsList>

            <div className="min-h-0 overflow-y-auto pr-2 custom-scrollbar">
              <TabsContent value="general" className="mt-0"><PreferenceSettings /></TabsContent>
              <TabsContent value="recording" className="mt-0"><RecordingSettings /></TabsContent>
              <TabsContent value="Transcriptionmodels" className="mt-0">
                <TranscriptSettings transcriptModelConfig={transcriptModelConfig} setTranscriptModelConfig={setTranscriptModelConfig} />
              </TabsContent>
              <TabsContent value="summaryModels" className="mt-0"><SummaryModelSettings /></TabsContent>
              <TabsContent value="beta" className="mt-0"><BetaSettings /></TabsContent>
            </div>
          </Tabs>
        </div>
      </div>
    </div>
  );
};
