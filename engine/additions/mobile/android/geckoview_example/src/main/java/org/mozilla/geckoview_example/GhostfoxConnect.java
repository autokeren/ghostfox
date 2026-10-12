/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package org.mozilla.geckoview_example;

import java.io.InputStream;
import java.io.OutputStream;
import java.net.Socket;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Ghostfox Connect: dial OUT to the MCP gateway (no Termux, no adb — the
 * phone initiates the connection, so dynamic wifi IPs don't matter). The
 * app sends "phone:<token>" and then bridges the gateway pipe to the
 * local juggler listener on 127.0.0.1:9222.
 */
public class GhostfoxConnect {
  public interface Listener {
    void onStatus(String msg);
  }

  private static final AtomicBoolean running = new AtomicBoolean(false);

  public static void connect(String serverHostPort, String token, Listener l) {
    if (!running.compareAndSet(false, true)) {
      l.onStatus("Already connecting — one bridge at a time");
      return;
    }
    final Thread t =
        new Thread(
            () -> {
              Socket vps = null;
              Socket local = null;
              try {
                String host = "SERVER_IP";
                int port = 9223;
                int ci = serverHostPort.lastIndexOf(':');
                if (ci > 0) {
                  host = serverHostPort.substring(0, ci).trim();
                  port = Integer.parseInt(serverHostPort.substring(ci + 1).trim());
                }
                l.onStatus("Connecting to " + host + ":" + port + " …");
                vps = new Socket(host, port);
                vps.setTcpNoDelay(true);
                vps.getOutputStream().write(("phone:" + token + "\0").getBytes("UTF-8"));
                vps.getOutputStream().flush();
                l.onStatus("Gateway OK — bridging to local juggler");
                local = new Socket("127.0.0.1", 9222);
                local.setTcpNoDelay(true);
                bridge(vps, local);
                l.onStatus("Bridge ended");
              } catch (Exception e) {
                l.onStatus("Error: " + e.getMessage());
              } finally {
                try {
                  if (vps != null) vps.close();
                } catch (Exception ignored) {
                }
                try {
                  if (local != null) local.close();
                } catch (Exception ignored) {
                }
                running.set(false);
              }
            });
    t.setDaemon(true);
    t.start();
  }

  private static void bridge(Socket a, Socket b) throws Exception {
    Thread t1 = new Thread(() -> copy(a, b));
    Thread t2 = new Thread(() -> copy(b, a));
    t1.setDaemon(true);
    t2.setDaemon(true);
    t1.start();
    t2.start();
    t1.join();
    t2.join();
  }

  private static void copy(Socket from, Socket to) {
    byte[] buf = new byte[16384];
    try {
      InputStream in = from.getInputStream();
      OutputStream out = to.getOutputStream();
      int n;
      while ((n = in.read(buf)) != -1) {
        out.write(buf, 0, n);
        out.flush();
      }
    } catch (Exception e) {
      try {
        from.close();
      } catch (Exception ignored) {
      }
      try {
        to.close();
      } catch (Exception ignored) {
      }
    }
  }
}
