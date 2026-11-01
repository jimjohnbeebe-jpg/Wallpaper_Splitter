# Manual test: v1 pass 1

These checks cover what the automated tests cannot: your real desktop, HDR, the full-screen windows, the file picker and drag and drop. The whole run takes about 15 minutes. Every step changes something visible, and the app puts HDR back when it closes. Setting the wallpaper does change your wallpaper, so note your current one if you want it back afterwards.

## Build and start

1. Open PowerShell and run these two commands. The build takes about 5 minutes the first time.

   ```powershell
   cd D:\Developer\WallPaper_Splitter
   npm run build
   ```

   You should see `Finished release profile` at the end.
2. Start the app:

   ```powershell
   .\src-tauri\target\release\WallpaperSplitter.exe
   ```

   You should see a dark window with Split, Monitors and Preferences on the left, and both monitors with an HDR switch in the footer.

## Checks

Note **pass** or **fail** for each, and a few words if something looks wrong.

1. **Open by picker.**
   - Do: Press **Ctrl+O** and pick `20220904-_DSC1541-HDR6000 x 4000ZephyrPhoto.jpg` from this folder.
   - Expect: The photo appears on the light table with two panes, LU28R55 on the left and BenQ EW3270U on the right. The Source panel shows "Ultra HDR JPEG" and an HDR badge. Amber "Enlarged" badges appear, because the photo is only 3840x1920.
2. **Drag and drop.**
   - Do: Drag any other photo from File Explorer onto the window.
   - Expect: It opens the same way.
3. **Monitor sizes.**
   - Do: Open the **Monitors** page.
   - Expect: Two panes drawn to scale, with the BenQ visibly larger. The LU28R55 reads 632x360 mm and the BenQ 698x393 mm. If those don't match the visible screen area you measure with a tape, type in the measured width and height.
4. **Identify.**
   - Do: Click **Identify**.
   - Expect: A large number and the monitor name appear on each monitor for about 2.5 seconds. The names match the physical monitors.
5. **HDR switch.**
   - Do: Click the BenQ's **HDR** switch in the footer, then click it again.
   - Expect: The BenQ flickers into HDR and back. The switch follows.
6. **Full-screen preview.**
   - Do: Reopen the HDR sample, go to **Split** and press **F11**.
   - Expect: Both monitors go full screen with the photo, and both switch into HDR. The photo continues across the gap, and the horizon is at the same height on both.
7. **Grid and nudge.**
   - Do: In the preview, press **G**, then **Tab**, then the arrow keys (hold **Shift** for 10 px).
   - Expect: A millimetre grid appears, and its lines continue straight across the bezel. Tab moves the selection label between monitors, and the arrow keys shift the selected monitor's picture.
8. **Leave the preview.**
   - Do: Press **Esc**.
   - Expect: The preview closes and both monitors return to HDR off. The nudge you made is kept in the Monitors panel on the Split page.
9. **In-app HDR.**
   - Do: Switch the BenQ's HDR on in the footer and look at the light table.
   - Expect: The sunset highlights look brighter than SDR white. Then switch HDR off again.
10. **Set wallpaper (PNG).**
    - Do: In Preferences, turn **HDR files for HDR images** off. Back on Split, click **Set wallpaper**.
    - Expect: The footer says "Wallpaper set on 2 monitors". Each desktop shows its part, and the picture lines up across the gap.
11. **Open folder.**
    - Do: Click **Open folder** in the footer.
    - Expect: File Explorer opens `Pictures\Wallpaper Splitter\<photo name>` with files named like `…_BenQ-EW3270U_3840x2160.png`.
12. **HDR wallpaper.**
    - Do: Turn **HDR files for HDR images** back on. Click **Set wallpaper**. Switch the BenQ's HDR on in Windows Settings > Display, then look at the desktop.
    - Expect: The files are `.jxr`, and the desktop sunset looks brighter than SDR white in HDR. Then switch HDR off again.
13. **Settings remembered.**
    - Do: Close the app and start it again with the command above.
    - Expect: The last photo, framing, monitor sizes and nudges come back.

## If something goes wrong

- If `npm run build` fails, copy the last 20 lines into the chat.
- If a check fails, write the check number and what you saw. A screenshot helps for anything visual.
- If HDR is still on after closing the app, turn it off in Settings > System > Display > Use HDR, and report it. It is meant to be restored.
- If Windows SmartScreen blocks the exe, it is unsigned. Click **More info**, then **Run anyway**.
