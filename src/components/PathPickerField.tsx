interface PathPickerFieldProps {
  label: string;
  value: string;
  emptyValueLabel: string;
  selectButtonLabel: string;
  isSelecting: boolean;
  isDisabled: boolean;
  onSelect: () => void;
  onClear?: () => void;
}

export function PathPickerField({
  label,
  value,
  emptyValueLabel,
  selectButtonLabel,
  isSelecting,
  isDisabled,
  onSelect,
  onClear,
}: PathPickerFieldProps) {
  const hasSelectedPath = value.trim().length > 0;

  return (
    <div className="file-picker-field">
      <span>{label}</span>
      <div className="file-picker-row">
        <button
          className="btn secondary"
          type="button"
          disabled={isDisabled}
          aria-label={`${selectButtonLabel}：${label}`}
          onClick={onSelect}
        >
          {isSelecting
            ? "正在选择…"
            : hasSelectedPath
              ? "重新选择"
              : selectButtonLabel}
        </button>
        {onClear && (
          <button
            className="btn ghost"
            type="button"
            disabled={isDisabled || !hasSelectedPath}
            aria-label={`清空${label}`}
            onClick={onClear}
          >
            清空
          </button>
        )}
        <div
          className={
            hasSelectedPath
              ? "file-picker-value"
              : "file-picker-value muted"
          }
          title={hasSelectedPath ? value : emptyValueLabel}
        >
          {hasSelectedPath ? value : emptyValueLabel}
        </div>
      </div>
    </div>
  );
}
