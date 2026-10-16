// This file is part of Loyalty Card App.
//
// Copyright (C) 2026 Marko Ivankovic
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

// The live scanner: the rear camera into a <video>, a frame every SCAN_INTERVAL_MS read by
// zxing, until a code is found. Frames are read at most FRAME_MAX pixels wide: enough for a
// card held near the phone, and quick on an old one.

import type { CodeFormat } from "./api";
import { scan } from "./barcode";

const SCAN_INTERVAL_MS = 120;
const FRAME_MAX = 1280;

export interface Found {
  format: CodeFormat;
  value: string;
}

export class Scanner {
  private stream: MediaStream | null = null;
  private readonly canvas = document.createElement("canvas");

  constructor(private readonly video: HTMLVideoElement) {}

  /** Asks for the camera. Throws with a message for the person if there is none or it is refused. */
  async start(): Promise<void> {
    if (!navigator.mediaDevices?.getUserMedia) {
      throw new Error("This browser cannot use the camera here.");
    }
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({
        audio: false,
        video: {
          facingMode: { ideal: "environment" },
          width: { ideal: 1920 },
          height: { ideal: 1080 },
        },
      });
    } catch {
      throw new Error("The camera is not available. Allow it in the browser's settings.");
    }
    this.video.srcObject = this.stream;
    this.video.setAttribute("playsinline", "");
    this.video.muted = true;
    await this.video.play();
  }

  /** Whether the camera has a light the app can turn on. */
  hasTorch(): boolean {
    const track = this.stream?.getVideoTracks()[0];
    const capabilities = track?.getCapabilities?.() as { torch?: boolean } | undefined;
    return capabilities?.torch === true;
  }

  async torch(on: boolean): Promise<void> {
    const track = this.stream?.getVideoTracks()[0];
    await track?.applyConstraints({ advanced: [{ torch: on } as MediaTrackConstraintSet] });
  }

  /** The first code in view. Rejects with an AbortError when `signal` aborts. */
  async next(signal: AbortSignal): Promise<Found> {
    const context = this.canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("This browser cannot read the camera's picture.");
    while (!signal.aborted) {
      const { videoWidth, videoHeight } = this.video;
      if (videoWidth > 0) {
        const scale = Math.min(1, FRAME_MAX / videoWidth);
        this.canvas.width = Math.round(videoWidth * scale);
        this.canvas.height = Math.round(videoHeight * scale);
        context.drawImage(this.video, 0, 0, this.canvas.width, this.canvas.height);
        const frame = context.getImageData(0, 0, this.canvas.width, this.canvas.height);
        const [found] = await scan(frame);
        if (found) return found;
      }
      await new Promise((resolve) => setTimeout(resolve, SCAN_INTERVAL_MS));
    }
    throw new DOMException("The scan was cancelled.", "AbortError");
  }

  stop(): void {
    for (const track of this.stream?.getTracks() ?? []) track.stop();
    this.stream = null;
    this.video.srcObject = null;
  }
}
