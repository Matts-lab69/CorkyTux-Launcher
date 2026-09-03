/*
 * CorkyTux - Java 25 Port
 * Copyright (C) 2026 queinu project / OnlineFix
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * Port from JPHP/DevelNext to pure Java 25 (Adoptium Temurin 25.0.4.1)
 * Original: https://github.com/onlinefix/linux-launcher
 */

package com.corkytux.launcher;

/**
 * Application entry point required for shaded runnable JAR.
 * <p>
 * Keeps {@code Main-Class} stable for {@code maven-shade-plugin} as
 * {@code com.corkytux.launcher.Main} while delegating to the JavaFX
 * {@link Launcher} subclass (which cannot be the manifest Main-Class directly
 * when shaded). Preserves Java 25 target and headless launch parity with
 * {@code Launcher.main()}.</p>
 */
public final class Main {

    private Main() {}

    /**
     * Delegates to {@link Launcher#main(String[])}.
     *
     * @param args command-line arguments, including optional game name for minimal mode
     */
    public static void main(String[] args) {
        Launcher.main(args);
    }
}
