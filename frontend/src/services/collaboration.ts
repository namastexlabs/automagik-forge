import { CollaborationEvent, UserPresence } from 'shared/types';

export type CollaborationEventHandler = (event: CollaborationEvent) => void;
export type PresenceEventHandler = (presence: UserPresence[]) => void;
export type ConnectionStatusHandler = (connected: boolean, error?: string) => void;

export interface CollaborationServiceConfig {
  projectId: string;
  authToken: string;
  onEvent?: CollaborationEventHandler;
  onPresenceUpdate?: PresenceEventHandler;
  onConnectionStatus?: ConnectionStatusHandler;
}

export class CollaborationService {
  private eventSource: EventSource | null = null;
  private presenceSource: EventSource | null = null;
  private config: CollaborationServiceConfig | null = null;
  private presenceInterval: number | null = null;
  private reconnectTimeout: number | null = null;
  private reconnectAttempts: number = 0;
  private maxReconnectAttempts: number = 10;
  private baseReconnectDelay: number = 1000;
  private isConnecting: boolean = false;
  private isDestroyed: boolean = false;

  constructor() {
    // Bind methods to preserve 'this' context
    this.handleEventSourceMessage = this.handleEventSourceMessage.bind(this);
    this.handleEventSourceError = this.handleEventSourceError.bind(this);
    this.handlePresenceSourceMessage = this.handlePresenceSourceMessage.bind(this);
    this.handlePresenceSourceError = this.handlePresenceSourceError.bind(this);
    this.sendHeartbeat = this.sendHeartbeat.bind(this);
  }

  /**
   * Connect to the collaboration system for a specific project
   */
  async connect(config: CollaborationServiceConfig): Promise<void> {
    if (this.isDestroyed) {
      throw new Error('CollaborationService has been destroyed');
    }

    // Disconnect any existing connections
    await this.disconnect();

    this.config = config;
    this.isConnecting = true;

    try {
      // Connect to event stream
      await this.connectEventStream();
      
      // Connect to presence stream
      await this.connectPresenceStream();

      // Start heartbeat for presence updates
      this.startHeartbeat();

      // Reset reconnect attempts on successful connection
      this.reconnectAttempts = 0;
      this.isConnecting = false;

      // Notify connection success
      this.config.onConnectionStatus?.(true);

      console.log('[CollaborationService] Connected to project:', config.projectId);
    } catch (error) {
      this.isConnecting = false;
      const errorMessage = error instanceof Error ? error.message : 'Unknown connection error';
      console.error('[CollaborationService] Connection failed:', errorMessage);
      this.config?.onConnectionStatus?.(false, errorMessage);
      
      // Schedule reconnection
      this.scheduleReconnect();
    }
  }

  /**
   * Disconnect from the collaboration system
   */
  async disconnect(): Promise<void> {
    console.log('[CollaborationService] Disconnecting...');

    // Clear reconnection timeout
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }

    // Clear heartbeat interval
    if (this.presenceInterval) {
      clearInterval(this.presenceInterval);
      this.presenceInterval = null;
    }

    // Close event source connections
    if (this.eventSource) {
      this.eventSource.removeEventListener('message', this.handleEventSourceMessage);
      this.eventSource.removeEventListener('error', this.handleEventSourceError);
      this.eventSource.close();
      this.eventSource = null;
    }

    if (this.presenceSource) {
      this.presenceSource.removeEventListener('message', this.handlePresenceSourceMessage);
      this.presenceSource.removeEventListener('error', this.handlePresenceSourceError);
      this.presenceSource.close();
      this.presenceSource = null;
    }

    // Notify disconnection
    this.config?.onConnectionStatus?.(false);
  }

  /**
   * Destroy the service and clean up all resources
   */
  destroy(): void {
    this.isDestroyed = true;
    this.disconnect();
    this.config = null;
  }

  /**
   * Update user presence status
   */
  async updatePresence(status: string): Promise<void> {
    if (!this.config) {
      throw new Error('Not connected to collaboration service');
    }

    try {
      const response = await fetch(`/api/projects/${this.config.projectId}/presence`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${this.config.authToken}`,
        },
        body: JSON.stringify({ status }),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }
    } catch (error) {
      console.error('[CollaborationService] Failed to update presence:', error);
      throw error;
    }
  }

  /**
   * Get current presence data for the project
   */
  async getCurrentPresence(): Promise<UserPresence[]> {
    if (!this.config) {
      throw new Error('Not connected to collaboration service');
    }

    try {
      const response = await fetch(`/api/projects/${this.config.projectId}/presence`, {
        headers: {
          'Authorization': `Bearer ${this.config.authToken}`,
        },
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      return data.data || [];
    } catch (error) {
      console.error('[CollaborationService] Failed to get current presence:', error);
      throw error;
    }
  }

  /**
   * Check if the service is connected
   */
  isConnected(): boolean {
    return !!(
      this.config &&
      this.eventSource?.readyState === EventSource.OPEN &&
      this.presenceSource?.readyState === EventSource.OPEN
    );
  }

  /**
   * Check if the service is currently connecting
   */
  isConnectingState(): boolean {
    return this.isConnecting;
  }

  private async connectEventStream(): Promise<void> {
    if (!this.config) throw new Error('No configuration available');

    const url = `/api/projects/${this.config.projectId}/events/stream`;
    
    this.eventSource = new EventSource(url, {
      withCredentials: true,
    });

    // Add custom headers for authentication (EventSource doesn't support custom headers directly)
    // The backend should handle authentication via cookies or query parameters for SSE
    
    this.eventSource.addEventListener('message', this.handleEventSourceMessage);
    this.eventSource.addEventListener('error', this.handleEventSourceError);

    // Wait for connection to open
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Event stream connection timeout'));
      }, 10000);

      this.eventSource!.addEventListener('open', () => {
        clearTimeout(timeout);
        resolve();
      }, { once: true });

      this.eventSource!.addEventListener('error', () => {
        clearTimeout(timeout);
        reject(new Error('Event stream connection failed'));
      }, { once: true });
    });
  }

  private async connectPresenceStream(): Promise<void> {
    if (!this.config) throw new Error('No configuration available');

    const url = `/api/projects/${this.config.projectId}/presence/stream`;
    
    this.presenceSource = new EventSource(url, {
      withCredentials: true,
    });

    this.presenceSource.addEventListener('message', this.handlePresenceSourceMessage);
    this.presenceSource.addEventListener('error', this.handlePresenceSourceError);

    // Wait for connection to open
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Presence stream connection timeout'));
      }, 10000);

      this.presenceSource!.addEventListener('open', () => {
        clearTimeout(timeout);
        resolve();
      }, { once: true });

      this.presenceSource!.addEventListener('error', () => {
        clearTimeout(timeout);
        reject(new Error('Presence stream connection failed'));
      }, { once: true });
    });
  }

  private handleEventSourceMessage(event: MessageEvent): void {
    try {
      const data = JSON.parse(event.data);
      
      // Validate the event structure
      if (data && typeof data.event_type === 'string' && data.project_id) {
        this.config?.onEvent?.(data as CollaborationEvent);
      } else {
        console.warn('[CollaborationService] Invalid event data:', data);
      }
    } catch (error) {
      console.error('[CollaborationService] Failed to parse event message:', error);
    }
  }

  private handleEventSourceError(event: Event): void {
    console.error('[CollaborationService] Event stream error:', event);
    
    if (this.eventSource?.readyState === EventSource.CLOSED) {
      this.config?.onConnectionStatus?.(false, 'Event stream connection lost');
      this.scheduleReconnect();
    }
  }

  private handlePresenceSourceMessage(event: MessageEvent): void {
    try {
      const data = JSON.parse(event.data);
      
      // Expect an array of UserPresence objects
      if (Array.isArray(data)) {
        this.config?.onPresenceUpdate?.(data as UserPresence[]);
      } else {
        console.warn('[CollaborationService] Invalid presence data:', data);
      }
    } catch (error) {
      console.error('[CollaborationService] Failed to parse presence message:', error);
    }
  }

  private handlePresenceSourceError(event: Event): void {
    console.error('[CollaborationService] Presence stream error:', event);
    
    if (this.presenceSource?.readyState === EventSource.CLOSED) {
      this.config?.onConnectionStatus?.(false, 'Presence stream connection lost');
      this.scheduleReconnect();
    }
  }

  private startHeartbeat(): void {
    // Send heartbeat every 30 seconds to maintain presence
    this.presenceInterval = setInterval(this.sendHeartbeat, 30000);
  }

  private async sendHeartbeat(): Promise<void> {
    if (!this.isConnected()) return;

    try {
      await this.updatePresence('Online');
    } catch (error) {
      console.error('[CollaborationService] Heartbeat failed:', error);
    }
  }

  private scheduleReconnect(): void {
    if (this.isDestroyed || this.reconnectTimeout || this.reconnectAttempts >= this.maxReconnectAttempts) {
      if (this.reconnectAttempts >= this.maxReconnectAttempts) {
        console.error('[CollaborationService] Max reconnect attempts reached. Entering offline mode.');
        this.config?.onConnectionStatus?.(false, 'Connection failed. Working in offline mode.');
      }
      return;
    }

    // Check if we're actually online
    if (!navigator.onLine) {
      console.log('[CollaborationService] Device is offline. Will retry when online.');
      this.config?.onConnectionStatus?.(false, 'Device is offline');
      
      // Listen for online event
      const handleOnline = () => {
        console.log('[CollaborationService] Device is back online. Attempting to reconnect.');
        window.removeEventListener('online', handleOnline);
        if (this.config && !this.isDestroyed) {
          this.reconnectAttempts = 0; // Reset attempts when back online
          this.scheduleReconnect();
        }
      };
      window.addEventListener('online', handleOnline);
      return;
    }

    const delay = Math.min(
      this.baseReconnectDelay * Math.pow(2, this.reconnectAttempts),
      30000 // Max 30 seconds
    );

    console.log(`[CollaborationService] Scheduling reconnect in ${delay}ms (attempt ${this.reconnectAttempts + 1}/${this.maxReconnectAttempts})`);

    this.reconnectTimeout = setTimeout(async () => {
      this.reconnectTimeout = null;
      this.reconnectAttempts++;

      if (this.config && !this.isDestroyed) {
        try {
          await this.connect(this.config);
        } catch (error) {
          console.error('[CollaborationService] Reconnect failed:', error);
          // Continue trying if we haven't hit max attempts
          if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.scheduleReconnect();
          }
        }
      }
    }, delay);
  }
}

// Singleton instance
export const collaborationService = new CollaborationService();