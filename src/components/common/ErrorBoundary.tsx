import React, { Component, ErrorInfo, ReactNode } from 'react';
import { AlertTriangle, RefreshCw } from 'lucide-react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Uncaught error in React tree:', error, errorInfo);
  }

  public handleReload = () => {
    window.location.reload();
  };

  public render() {
    if (this.state.hasError) {
      return (
        <div className="flex h-screen w-screen flex-col items-center justify-center bg-slate-50 p-8 text-center select-none">
          <div className="max-w-md w-full rounded-2xl bg-white p-8 border border-slate-200 shadow-sm flex flex-col items-center">
            <div className="w-12 h-12 rounded-2xl bg-amber-50 text-amber-600 flex items-center justify-center mb-4 border border-amber-200/60">
              <AlertTriangle className="w-6 h-6" />
            </div>
            <h3 className="text-base font-bold text-slate-800 mb-1">
              界面渲染遇到异常
            </h3>
            <p className="text-xs text-slate-500 mb-4 max-w-xs leading-relaxed">
              组件已进行故障隔离，未影响后台进程。请点击下方按钮重新载入界面。
            </p>
            {this.state.error && (
              <div className="w-full text-left bg-slate-50 p-3 rounded-xl border border-slate-200 text-[11px] font-mono text-slate-600 mb-5 max-h-32 overflow-y-auto break-all">
                {this.state.error.message}
              </div>
            )}
            <button
              onClick={this.handleReload}
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-xl bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-semibold shadow-xs transition-colors"
            >
              <RefreshCw className="w-3.5 h-3.5" />
              <span>重新载入界面</span>
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
