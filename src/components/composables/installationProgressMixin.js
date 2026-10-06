/**
 * Options API mixin with the progress throttling and virtual-scrolled log
 * state shared by `InstalationProgress.vue` (wizard / repair) and
 * `OfflineInstaller.vue`. The log panel itself is rendered by
 * `InstallationLog.vue`, which must be given `ref="virtualContainer"`.
 *
 * The host component owns `this._progressData` (its fallback activity text
 * differs per flow) and the `currentActivity` computed.
 */

const MAX_LOG_ENTRIES = 1000;

export const installationProgressMixin = {
  data() {
    return {
      // Logging with virtual scrolling
      visibleLogs: [],
      totalLogCount: 0,
      scrollTop: 0,
      containerHeight: 300,
      itemHeight: 24,
      visibleCount: 15,
      startIndex: 0,

      // UI state
      installationPath: "",

      // Progress tracking
      progressUpdateTrigger: 0,
      lastProgressUpdate: 0,
    };
  },

  created() {
    this._allLogs = [];
    this.BUFFER_SIZE = 2;
    this._progressThrottle = null;
  },

  computed: {
    topSpacerHeight() {
      return this.startIndex * this.itemHeight;
    },

    bottomSpacerHeight() {
      const remainingItems = Math.max(0, this.totalLogCount - (this.startIndex + this.visibleLogs.length));
      return remainingItems * this.itemHeight;
    },

    logListProps() {
      return {
        logs: this.visibleLogs,
        startIndex: this.startIndex,
        itemHeight: this.itemHeight,
        topSpacerHeight: this.topSpacerHeight,
        bottomSpacerHeight: this.bottomSpacerHeight,
      };
    },

    maxVisibleItems() {
      return Math.ceil(this.containerHeight / this.itemHeight) + (this.BUFFER_SIZE * 2);
    },

    currentProgress() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentProgress : 0;
    },

    currentDetail() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentDetail : "";
    },
  },

  methods: {
    throttledProgressUpdate() {
      if (this._progressThrottle) {
        clearTimeout(this._progressThrottle);
      }

      this._progressThrottle = setTimeout(() => {
        const now = Date.now();
        if (now - this.lastProgressUpdate > 100) {
          this.progressUpdateTrigger++;
          this.lastProgressUpdate = now;
        }
        this._progressThrottle = null;
      }, 100);
    },

    handleLogMessage(payload) {
      const { level, message } = payload;

      const logEntry = {
        level,
        text: message,
        timestamp: Date.now(),
        id: this._allLogs.length
      };

      this._allLogs.unshift(logEntry);

      if (this._allLogs.length > MAX_LOG_ENTRIES) {
        this._allLogs = this._allLogs.slice(0, MAX_LOG_ENTRIES);
      }

      this.totalLogCount = this._allLogs.length;
      this.updateVisibleLogs();

      if (this.scrollTop < this.itemHeight) {
        this.scrollToTop();
      }

      // Extract installation path from logs if available
      if (message.includes('installed at:') || message.includes('Location:')) {
        const pathMatch = message.match(/(?:installed at:|Location:)\s*(.+)/i);
        if (pathMatch && pathMatch[1]) {
          this.installationPath = pathMatch[1].trim();
        }
      }
    },

    updateVisibleLogs() {
      const startIndex = Math.max(0, Math.floor(this.scrollTop / this.itemHeight) - this.BUFFER_SIZE);
      const endIndex = Math.min(
        startIndex + this.maxVisibleItems,
        this._allLogs.length
      );

      this.startIndex = startIndex;
      this.visibleLogs = this._allLogs.slice(startIndex, endIndex).map(log => ({
        ...log
      }));
    },

    resetLogs() {
      this._allLogs = [];
      this.visibleLogs = [];
      this.totalLogCount = 0;
      this.scrollTop = 0;
      this.startIndex = 0;
    },

    logScrollContainer() {
      return this.$refs.virtualContainer?.$refs.container;
    },

    onLogScroll(event) {
      const newScrollTop = event.target.scrollTop;

      if (Math.abs(newScrollTop - this.scrollTop) > this.itemHeight / 2) {
        this.scrollTop = newScrollTop;
        this.updateVisibleLogs();
      }
    },

    scrollToTop() {
      this.$nextTick(() => {
        const container = this.logScrollContainer();
        if (container) {
          container.scrollTop = 0;
          this.scrollTop = 0;
          this.updateVisibleLogs();
        }
      });
    },

    measureContainer() {
      this.$nextTick(() => {
        const container = this.logScrollContainer();
        if (container) {
          this.containerHeight = container.clientHeight;
          this.updateVisibleLogs();
        }
      });
    },
  },
};
