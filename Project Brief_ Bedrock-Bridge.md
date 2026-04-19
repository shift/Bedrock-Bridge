# **Project Brief: Bedrock-Bridge**

## **1\. Objective**

Develop a Tauri v2 application (Desktop & Android) that acts as a UDP relay for Minecraft Bedrock Edition. The application allows users to store a list of remote server profiles (Host/Port tuples) and activate one to proxy local console traffic to that remote destination.

## **2\. Functional Requirements**

### **A. Profile Management**

* **CRUD Operations:** Users must be able to add, edit, and delete profiles.  
* **Data Model:** Each profile contains a Label, Remote Host, and Remote Port.  
* **Persistence:** Profiles must be saved locally (e.g., using tauri-plugin-store or a local JSON file).  
* **Activation:** A single profile can be "Active" at any time. Activating a profile starts the proxy services.

### **B. Discovery Service (UDP 19132\)**

* **Listen:** Bind to 0.0.0.0:19132 (UDP).  
* **Respond:** When receiving a 0x01 (Unconnected Ping), reply with 0x1c (Unconnected Pong).  
* **Dynamic MOTD:** The Pong packet must use the Label of the active profile as the server name in the MOTD string.

### **C. UDP Proxy Logic**

* **Transparent Relay:** Forward all non-discovery UDP packets from the local source to the selected Remote Host:Port.  
* **Bidirectional Session Mapping:** Maintain a map to route response packets from the remote host back to the correct local client.  
* **MTU Management:** Intercept RakNet connection packets to ensure the MTU does not exceed 1400 bytes.

### **D. Android Implementation**

* **MulticastLock:** Use a native Kotlin bridge to acquire WifiManager.MulticastLock for discovery packet reception.  
* **Foreground Service:** Run the proxy as a Foreground Service with a persistent notification.  
* **WakeLock:** Maintain network connectivity when the device is idle/screen-off.

## **3\. Technical Specifications**

### **Handshake Protocol (RakNet)**

* **Magic Bytes:** 00 ff ff 00 fe fe fe fe fd fd fd fd 12 34 56 78  
* **Ping ID:** 0x01 | **Pong ID:** 0x1c

### **UI/UX Flow**

1. **List View:** Display all saved profiles with a toggle switch to activate the bridge.  
2. **Add/Edit View:** Form to input connection details.  
3. **Active State:** Show real-time traffic indicators (PPS/Throughput) for the running proxy.

## **4\. Success Criteria**

1. **Visibility:** Consoles detect the LAN game immediately upon activation.  
2. **Persistence:** The profile list survives app restarts.  
3. **Android Stability:** The proxy remains active during extended play sessions on mobile.