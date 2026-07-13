import React from 'react';
import Image from 'next/image';
import { ArrowRight, Link2, LockKeyhole, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';

export function WelcomeStep() {
  const { goNext } = useOnboarding();

  const features = [
    {
      icon: LockKeyhole,
      title: 'Private by architecture',
      description: 'Meetings, transcripts, and durable memory stay on this device.',
    },
    {
      icon: Link2,
      title: 'Every memory keeps its source',
      description: 'Jump from a decision or promise back to where it was said.',
    },
    {
      icon: RotateCcw,
      title: 'Built for the next conversation',
      description: 'Bring confirmed decisions and open loops back before you meet again.',
    },
  ];

  return (
    <OnboardingContainer
      title="Let's start Briefli"
      description="Set up your private record of what was decided, promised, and left unresolved."
      step={1}
      hideProgress={true}
    >
      <div className="grid gap-10 md:grid-cols-[1fr_240px]">
        <div className="space-y-1 border-t border-briefli-line pt-3">
          {features.map((feature, index) => {
            const Icon = feature.icon;
            return (
              <div key={index} className="flex items-start gap-4 border-b border-briefli-line py-5">
                <div className="mt-0.5 flex h-8 w-8 flex-shrink-0 items-center justify-center rounded bg-[#202621] text-white">
                  <Icon className="h-4 w-4" />
                </div>
                <div>
                  <p className="text-sm font-semibold text-briefli-ink">{feature.title}</p>
                  <p className="mt-1 text-sm leading-6 text-briefli-muted">{feature.description}</p>
                </div>
              </div>
            );
          })}
          <Button
            onClick={goNext}
            className="mt-7 h-11 rounded bg-[#202621] px-5 text-white hover:bg-black"
          >
            Set up Briefli <ArrowRight className="ml-2 h-4 w-4" />
          </Button>
        </div>
        <div className="flex min-h-72 flex-col justify-between rounded border border-briefli-line bg-briefli-sidebar p-6">
          <Image src="/briefli-mark.svg" alt="" width={56} height={56} />
          <p className="font-brand text-xl leading-8 text-briefli-ink">
            “Before my next conversation, remind me what we agreed.”
          </p>
          <p className="text-xs font-semibold uppercase tracking-[0.12em] text-briefli-muted">Setup takes a few minutes</p>
        </div>
      </div>
    </OnboardingContainer>
  );
}
