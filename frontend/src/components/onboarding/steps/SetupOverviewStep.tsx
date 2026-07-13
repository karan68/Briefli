import React, { useEffect, useState } from 'react';
import { ArrowRight, FileAudio, Info, MessageSquareText } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

export function SetupOverviewStep() {
  const { goNext } = useOnboarding();
  const [isMac, setIsMac] = useState(false);

  useEffect(() => {
    const checkPlatform = async () => {
      try {
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch (e) {
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
  }, []);

  const steps = [
    {
      number: 1,
      type: 'transcription',
      title: 'Private transcription',
      description: 'Turns speech into a searchable transcript on this device.',
      icon: FileAudio,
    },
    {
      number: 2,
      type: 'summarization',
      title: 'Meeting memory',
      description: 'Suggests decisions, commitments, and open questions for your review.',
      icon: MessageSquareText,
    },
  ];

  const handleContinue = () => {
    goNext();
  };

  return (
    <OnboardingContainer
      title="Prepare your local workspace"
      description="Briefli downloads two local capabilities. You can change models or connect an external provider later in Settings."
      step={2}
      totalSteps={isMac ? 4 : 3}
    >
      <div className="space-y-8">
        {/* Steps Card */}
        <div className="border-y border-briefli-line">
          <div>
            {steps.map((step, idx) => {
              const Icon = step.icon;
              return (
                <div
                  key={step.number}
                  className="grid grid-cols-[44px_1fr_auto] items-start gap-4 border-b border-briefli-line py-5 last:border-b-0"
                >
                  <div className="flex h-10 w-10 items-center justify-center rounded bg-[#202621] text-white"><Icon className="h-5 w-5" /></div>
                  <div>
                    <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-briefli-muted">Step {step.number}</p>
                    <h3 className="mt-1 flex items-center gap-2 font-semibold text-briefli-ink">
                        {step.title}

                        {step.type === "summarization" && (
                            <TooltipProvider>
                            <Tooltip>
                                <TooltipTrigger asChild>
                                <button className="text-gray-400 hover:text-gray-600">
                                    <Info className="w-4 h-4" />
                                </button>
                                </TooltipTrigger>
                                <TooltipContent className="max-w-xs text-sm">
                                You can also select external AI providers like OpenAI, Claude, or
                                Ollama for summary generation in settings.
                                </TooltipContent>
                            </Tooltip>
                            </TooltipProvider>
                        )}
                        </h3>
                        <p className="mt-1 text-sm leading-6 text-briefli-muted">{step.description}</p>
                  </div>
                      <span className="pt-2 text-xs text-briefli-muted">Local</span>
                </div>
              );
            })}
          </div>
        </div>


        {/* CTA Section */}
        <div>
          <Button
            onClick={handleContinue}
            className="h-11 rounded bg-[#202621] px-5 text-white hover:bg-black"
          >
            Download and continue <ArrowRight className="ml-2 h-4 w-4" />
          </Button>
          <p className="mt-3 text-left text-xs text-briefli-muted">Downloads are stored in Briefli&apos;s application data folder.</p>
        </div>
      </div>
    </OnboardingContainer>
  );
}
