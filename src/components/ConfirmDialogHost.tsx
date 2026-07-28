import { useEffect, useId, useRef, useState } from "react";
import {
  registerConfirmHost,
  type ConfirmRequest,
} from "../confirmAction";

/**
 * In-app confirm dialog host. Registers itself as the global confirmAction
 * backend so call sites can `await confirmAction(...)` without depending on
 * browser/native dialogs that break the product visual language.
 */
export function ConfirmDialogHost() {
  const [activeRequest, setActiveRequest] = useState<ConfirmRequest | null>(
    null,
  );
  const queueRef = useRef<ConfirmRequest[]>([]);
  const confirmButtonRef = useRef<HTMLButtonElement | null>(null);
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    const presentNextRequest = () => {
      setActiveRequest((currentRequest) => {
        if (currentRequest != null) {
          return currentRequest;
        }
        return queueRef.current.shift() ?? null;
      });
    };

    registerConfirmHost((request) => {
      queueRef.current.push(request);
      presentNextRequest();
    });

    return () => {
      registerConfirmHost(null);
      // Reject any queued prompts if the host unmounts mid-session.
      const pendingRequests = queueRef.current.splice(0);
      for (const pendingRequest of pendingRequests) {
        pendingRequest.resolve(false);
      }
      setActiveRequest((currentRequest) => {
        currentRequest?.resolve(false);
        return null;
      });
    };
  }, []);

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

    const finishRequest = (confirmed: boolean) => {
      setActiveRequest((currentRequest) => {
        if (currentRequest == null) {
          return null;
        }
        currentRequest.resolve(confirmed);
        return queueRef.current.shift() ?? null;
      });
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        finishRequest(false);
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

  const resolveRequest = (confirmed: boolean) => {
    setActiveRequest((currentRequest) => {
      if (currentRequest == null) {
        return null;
      }
      currentRequest.resolve(confirmed);
      return queueRef.current.shift() ?? null;
    });
  };

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
