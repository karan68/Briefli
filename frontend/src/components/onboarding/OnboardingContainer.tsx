import React from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { cn } from '@/lib/utils';
import { ProgressIndicator } from './shared/ProgressIndicator';
import { useOnboarding } from '@/contexts/OnboardingContext';
import type { OnboardingContainerProps } from '@/types/onboarding';

export function OnboardingContainer({
  title,
  description,
  children,
  step,
  totalSteps = 5,
  stepOffset = 0,
  hideProgress = false,
  className,
  showNavigation = false,
  onNext,
  onPrevious,
  canGoNext = true,
  canGoPrevious = true,
}: OnboardingContainerProps) {
  const { goToStep, goPrevious, goNext } = useOnboarding();

  const handlePrevious = () => {
    if (onPrevious) {
      onPrevious();
    } else {
      goPrevious();
    }
  };

  const handleNext = () => {
    if (onNext) {
      onNext();
    } else {
      goNext();
    }
  };

  const handleStepClick = (s: number) => {
    goToStep(s + stepOffset);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center overflow-hidden bg-briefli-paper">
      <div className="pointer-events-none absolute inset-y-0 left-0 w-2 bg-[#202621]" />
      <div className={cn('flex h-full max-h-screen w-full max-w-3xl flex-col px-10 py-8', className)}>
        {/* Progress Indicator with Navigation - Fixed */}
        {step && !hideProgress && (
          <div className="mb-2 relative flex-shrink-0">
            {/* Navigation Buttons */}
            {showNavigation && (
              <div className="absolute top-1/2 -translate-y-1/2 left-0 right-0 flex justify-between pointer-events-none">
                <button
                  onClick={handlePrevious}
                  disabled={!canGoPrevious || step === 1}
                  className={cn(
                    'pointer-events-auto w-8 h-8 rounded-full bg-white border border-gray-200 shadow-sm flex items-center justify-center transition-all duration-200',
                    canGoPrevious && step !== 1
                      ? 'hover:bg-gray-50 hover:shadow-md hover:scale-110 text-gray-700'
                      : 'opacity-0 cursor-not-allowed'
                  )}
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>

                <button
                  onClick={handleNext}
                  disabled={!canGoNext || step === totalSteps}
                  className={cn(
                    'pointer-events-auto w-8 h-8 rounded-full bg-white border border-gray-200 shadow-sm flex items-center justify-center transition-all duration-200',
                    canGoNext && step !== totalSteps
                      ? 'hover:bg-gray-50 hover:shadow-md hover:scale-110 text-gray-700'
                      : 'opacity-0 cursor-not-allowed'
                  )}
                >
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>
            )}

            {/* Progress Indicator */}
            <ProgressIndicator current={step} total={totalSteps} onStepClick={handleStepClick} />
          </div>
        )}

        {/* Header - Fixed */}
        <div className="mb-8 flex-shrink-0 space-y-3 text-left">
          <h1 className="animate-fade-in-up font-brand text-[42px] font-semibold leading-tight text-briefli-ink">{title}</h1>
          {description && (
            <p className="max-w-xl animate-fade-in-up text-base leading-7 text-briefli-muted delay-75">
              {description}
            </p>
          )}
        </div>

        {/* Content - Scrollable */}
        <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
          <div className="space-y-6">{children}</div>
        </div>
      </div>
    </div>
  );
}
