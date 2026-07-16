import React from 'react';

interface ConfirmationModalProps {
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
  text: string;
  isOpen: boolean;
  title?: string;
  confirmLabel?: string;
  isConfirming?: boolean;
}

export function ConfirmationModal({
  onConfirm,
  onCancel,
  text,
  isOpen,
  title = 'Confirm Delete',
  confirmLabel = 'Delete',
  isConfirming = false,
}: ConfirmationModalProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg p-6 max-w-md w-full mx-4">
        <h2 className="text-xl font-semibold mb-4">{title}</h2>
        <p className="text-gray-600 mb-6">{text}</p>
        <div className="flex justify-end space-x-4">
          <button
            onClick={onCancel}
            disabled={isConfirming}
            className="px-4 py-2 text-gray-600 hover:bg-gray-100 rounded-md transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={isConfirming}
            className="px-4 py-2 bg-red-600 text-white hover:bg-red-700 rounded-md transition-colors disabled:opacity-50"
          >
            {isConfirming ? `${confirmLabel}...` : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
