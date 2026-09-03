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

package com.corkytux.launcher.modules;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * Java 25 port of {@code Localization.php} (28 lines).
 *
 * <p>PHP original:</p>
 * <pre>
 * function doConstruct(ScriptEvent $e = null){
 *   $locale = Locale::getDefault()->getLanguage();
 *   if (ResourceStream::exists('res://locale/'.$locale.'.json') == false) $locale = 'en';
 *   $this->strings = Json::decode(ResourceStream::of('res://locale/'.$locale.'.json')->readFully());
 * }
 * public function _(string $code){ return $this->strings[$code] ?? "FAILED TO LOAD $code"; }
 * </pre>
 *
 * <p>Java maps {@code res://locale/…} to classpath {@code /locale/…} (see
 * {@code src/main/resources/locale/en.json}, {@code ru.json}). Jackson's
 * {@link ObjectMapper} replaces {@code facade\Json}. Lookup is case-sensitive
 * and returns the same fallback sentinel as PHP.</p>
 */
public final class Localization {

    private static final Logger LOG = LoggerFactory.getLogger(Localization.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final TypeReference<Map<String, String>> MAP_TYPE = new TypeReference<>() {};

    private static volatile Localization instance;

    private final Map<String, String> strings;
    private final String locale;

    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code @event construct} / {@code doConstruct()}.
     * Resolves {@link Locale#getDefault()}'s language, falls back to {@code en}
     * when the resource does not exist.
     */
    public Localization() {
        String lang = Locale.getDefault().getLanguage();
        if (lang == null || lang.isBlank()) lang = "en";
        LOG.debug("System locale language: {}", lang);

        String resolved = lang;
        if (!resourceExists("/locale/" + lang + ".json")) {
            LOG.info("Locale {} not found, falling back to en", lang);
            resolved = "en";
        }
        this.locale = resolved;
        this.strings = loadStrings(resolved);
        LOG.info("Localization loaded: locale={}, keys={}", locale, strings.size());
    }

    /**
     * Alternate constructor for explicit locale – useful in tests.
     */
    public Localization(String explicitLocale) {
        String resolved = explicitLocale;
        if (!resourceExists("/locale/" + explicitLocale + ".json")) {
            LOG.info("Locale {} not found, falling back to en", explicitLocale);
            resolved = "en";
        }
        this.locale = resolved;
        this.strings = loadStrings(resolved);
    }

    // -----------------------------------------------------------------------
    // Singleton
    // -----------------------------------------------------------------------

    public static Localization getInstance() {
        if (instance == null) {
            synchronized (Localization.class) {
                if (instance == null) instance = new Localization();
            }
        }
        return instance;
    }

    /**
     * For tests: reset singleton.
     */
    static void resetInstance() {
        instance = null;
    }

    // -----------------------------------------------------------------------
    // Core API – mirrors PHP _()
    // -----------------------------------------------------------------------

    /**
     * Mirrors PHP {@code _(string $code)}.
     *
     * @param code localization key (e.g. {@code "APPMODULE.PIDEXISTS"})
     * @return localized string or {@code "FAILED TO LOAD <code>"} sentinel
     */
    public String get(String code) {
        String val = strings.get(code);
        if (val == null) {
            LOG.debug("Missing localization key: {}", code);
            return "FAILED TO LOAD " + code;
        }
        return val;
    }

    /**
     * Alias for PHP's {@code _()} name – Java cannot name a method {@code _} alone
     * since Java 9 (identifier), so we expose {@code underscore()}.
     * Use {@link #get(String)} as primary entry.
     */
    public String underscore(String code) {
        return get(code);
    }

    /**
     * Varargs formatter mirroring PHP {@code sprintf(Localization::_('KEY'), ...)} pattern.
     */
    public String format(String code, Object... args) {
        return String.format(get(code), args);
    }

    public String getLocale() {
        return locale;
    }

    public Map<String, String> getAllStrings() {
        return Collections.unmodifiableMap(strings);
    }

    // -----------------------------------------------------------------------
    // Internal – loading
    // -----------------------------------------------------------------------

    private static boolean resourceExists(String path) {
        return Localization.class.getResourceAsStream(path) != null;
    }

    private static Map<String, String> loadStrings(String locale) {
        String resourcePath = "/locale/" + locale + ".json";
        try (InputStream is = Localization.class.getResourceAsStream(resourcePath)) {
            if (is == null) {
                LOG.error("Resource not found: {}", resourcePath);
                // try en as ultimate fallback
                if (!"en".equals(locale)) {
                    try (InputStream en = Localization.class.getResourceAsStream("/locale/en.json")) {
                        if (en != null) return MAPPER.readValue(en, MAP_TYPE);
                    }
                }
                return Map.of();
            }
            // Read fully – mirrors ResourceStream->readFully()
            byte[] bytes = is.readAllBytes();
            String json = new String(bytes, StandardCharsets.UTF_8);
            LOG.debug("Loaded locale JSON {} ({} bytes)", resourcePath, bytes.length);
            Map<String, String> map = MAPPER.readValue(json, MAP_TYPE);
            return new HashMap<>(map);
        } catch (IOException e) {
            LOG.error("Failed to decode locale JSON {}", resourcePath, e);
            return Map.of();
        }
    }
}
