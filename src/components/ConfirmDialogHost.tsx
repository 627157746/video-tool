import { useEffect, useId, useRef, useState } from "react";
import {
  registerConfirmHost,
  type ConfirmRequest,
} from "../confirmAction";

/**
 * In-app confirm dialog host.
 *
 * Queue mutations must stay outside React state updaters: StrictMode double-
 * invokes updaters in development, and `queue.shift()` inside setState would
 * drop the first confirm (user has to click delete twice).
 */
export function ConfirmDialogHost() {
  const [activeRequest, setActiveRequest] = useState<ConfirmRequest | null>(
    null,
  );
  const queueRef = useRef<ConfirmRequest[]>([]);
  const activeRequestRef = useRef<ConfirmRequest | null>(null);
  const isResolvingRef = useRef(false);
  const confirmButtonRef = useRef<HTMLButtonElement | null>(null);
  const titleId = useId();
  const descriptionId = useId();

  const syncActiveFromQueue = () => {
    const nextActiveRequest = queueRef.current[0] ?? null;
    activeRequestRef.current = nextActiveRequest;
    setActiveRequest(nextActiveRequest);
  };

  useEffect(() => {
    const enqueueRequest = (request: ConfirmRequest) => {
      queueRef.current.push(request);
      // Only promote when nothing is currently shown.
      if (activeRequestRef.current == null) {
        syncActiveFromQueue();
      }
    };

    registerConfirmHost(enqueueRequest);

    return () => {
      // StrictMode remounts effects; only clear if we are still the active host.
      registerConfirmHost(null, enqueueRequest);
      const pendingRequests = queueRef.current.splice(0);
      activeRequestRef.current = null;
      isResolvingRef.current = false;
      for (const pendingRequest of pendingRequests) {
        pendingRequest.resolve(false);
      }
      setActiveRequest(null);
    };
  }, []);

  const resolveRequest = (confirmed: boolean) => {
    if (isResolvingRef.current) {
      return;
    }
    const currentRequest = activeRequestRef.current ?? queueRef.current[0];
    if (currentRequest == null) {
      return;
    }
    isResolvingRef.current = true;
    try {
      if (queueRef.current[0] === currentRequest) {
        queueRef.current.shift();
      } else {
        const requestIndex = queueRef.current.indexOf(currentRequest);
        if (requestIndex >= 0) {
          queueRef.current.splice(requestIndex, 1);
        }
      }
      activeRequestRef.current = null;
      currentRequest.resolve(confirmed);
      syncActiveFromQueue();
    } finally {
      isResolvingRef.current = false;
    }
  };

  useEffect(() => {
    if (activeRequest == null) {
      return;
    }

    const previouslyFocusedElement =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;

    const focusTimer = window.setTimeout(() => {
      confirmButtonRef.current?.focus();
    }, 0);

    const backgroundElementStates = new Map<HTMLElement, boolean>();
    const makeBackgroundInert = () => {
      const backgroundElements = document.querySelectorAll<HTMLElement>(
        ".topbar, .content, .modal-backdrop:not(.confirm-dialog-backdrop)",
      );
      backgroundElements.forEach((element) => {
        if (!backgroundElementStates.has(element)) {
          backgroundElementStates.set(element, element.inert);
        }
        element.inert = true;
      });
    };
    makeBackgroundInert();
    const backgroundObserver = new MutationObserver(makeBackgroundInert);
    backgroundObserver.observe(document.body, {
      childList: true,
      subtree: true,
    });

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        resolveRequest(false);
        return;
      }
      if (event.key !== "Tab") {
        return;
      }
      const dialog = document.querySelector<HTMLElement>(
        ".confirm-dialog[role='dialog']",
      );
      const focusableElements = Array.from(
        dialog?.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      );
      if (focusableElements.length === 0) {
        return;
      }
      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];
      if (event.shiftKey && document.activeElement === firstElement) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && document.activeElement === lastElement) {
        event.preventDefault();
        firstElement.focus();
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener("keydown", handleKeyDown, true);
      backgroundObserver.disconnect();
      backgroundElementStates.forEach((wasInert, element) => {
        element.inert = wasInert;
      });
      previouslyFocusedElement?.focus?.();
    };
  }, [activeRequest]);

  if (activeRequest == null) {
    return null;
  }

  const title = activeRequest.options.title?.trim() || "请确认";
  const okLabel = activeRequest.options.okLabel?.trim() || "确定";
  const cancelLabel = activeRequest.options.cancelLabel?.trim() || "取消";
  const kind = activeRequest.options.kind ?? "warning";
  const isDestructive = kind === "warning" || kind === "error";
  const kickerLabel =
    kind === "error" ? "危险操作" : kind === "warning" ? "需要确认" : "提示";

  return (
    <div
      className="modal-backdrop confirm-dialog-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          resolveRequest(false);
        }
      }}
    >
      <div
        className={`modal confirm-dialog confirm-dialog--${kind}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <div className="modal-header confirm-dialog-header">
          <div>
            <div className="detail-kicker">{kickerLabel}</div>
            <h2 id={titleId}>{title}</h2>
          </div>
        </div>

        <p id={descriptionId} className="confirm-dialog-message">
          {activeRequest.message}
        </p>

        <div className="modal-actions confirm-dialog-actions">
          <button
            type="button"
            className="btn secondary"
            onClick={() => resolveRequest(false)}
          >
            {cancelLabel}
          </button>
          <button
            ref={confirmButtonRef}
            type="button"
            className={isDestructive ? "btn danger" : "btn"}
            onClick={() => resolveRequest(true)}
          >
            {okLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
