'use client';

import { motion } from 'framer-motion';
import { FileQuestion, Sparkles } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';

interface EmptyStateSummaryProps {
  onGenerate: () => void;
  hasModel: boolean;
  isGenerating?: boolean;
  showAction?: boolean;
}

export function EmptyStateSummary({ onGenerate, hasModel, isGenerating = false, showAction = true }: EmptyStateSummaryProps) {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      className="flex flex-col items-center justify-center h-full p-8 text-center"
    >
      <FileQuestion className="w-16 h-16 text-gray-300 mb-4" />
      <h3 className="text-lg font-semibold text-gray-900 mb-2">
        No meeting record yet
      </h3>
      <p className="text-sm text-gray-500 mb-6 max-w-md">
        Build a source-backed record of the key decisions, commitments, and open questions in this conversation.
      </p>

      {showAction && <TooltipProvider>
        <Tooltip>
          <TooltipTrigger asChild>
            <div>
              <Button
                onClick={onGenerate}
                disabled={!hasModel || isGenerating}
                className="gap-2"
              >
                <Sparkles className="w-4 h-4" />
                {isGenerating ? 'Generating...' : 'Generate Summary'}
              </Button>
            </div>
          </TooltipTrigger>
          {!hasModel && (
            <TooltipContent>
              <p>Please select a model in Settings first</p>
            </TooltipContent>
          )}
        </Tooltip>
      </TooltipProvider>}

      {!hasModel && (
        <p className="text-xs text-amber-600 mt-3">
          Please select a model in Settings first
        </p>
      )}
    </motion.div>
  );
}
