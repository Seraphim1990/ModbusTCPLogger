// =====================================================
// schemas.js — конфіги для динамічних форм value_unit
// Щоб додати новий тип: додай новий запис в VALUE_SCHEMAS
// =====================================================

const VALUE_SCHEMAS = {

    // Тип 1: ComaShift — одиночний регістр
    1: {
        name: 'ComaShift',
        fields: [
            {
                key:      'addr',
                label:    'Адреса регістру',
                type:     'number',
                required: true,
                min:      0,
                max:      65535,
            },
            {
                key:      'regType',
                label:    'Тип регістру',
                type:     'select',
                required: true,
                options:  ['holding', 'input', 'coil'],
            },
            {
                key:      'multiplier',
                label:    'Множник',
                type:     'number',
                required: true,
                default:  1,
            },
        ]
    },

    // Тип 2: SatecDoubleRegistersInt32 — подвійний регістр (унікальний SATEC)
    2: {
        name: 'SatecDoubleRegistersInt32',
        fields: [
            {
                key:      'hiRegister',
                label:    'Hi регістр',
                type:     'number',
                required: true,
                min:      0,
                max:      65535,
            },
            {
                key:      'loRegister',
                label:    'Lo регістр',
                type:     'number',
                required: true,
                min:      0,
                max:      65535,
            },
            {
                key:      'regType',
                label:    'Тип регістру',
                type:     'select',
                required: true,
                options:  ['holding', 'input'],
            },
            {
                key:     'isSigned',
                label:   'Знакове число',
                type:    'checkbox',
                default: false,
            },
            {
                key:      'multiplier',
                label:    'Множник',
                type:     'number',
                required: true,
                default:  1,
            },
        ]
    },

    // Тип 3: BitInWord — бітова адресація
    3: {
        name: 'BitInWord',
        fields: [
            {
                key:      'addr',
                label:    'Адреса регістру',
                type:     'number',
                required: true,
                min:      0,
                max:      65535,
            },
            {
                key:      'bitAddr',
                label:    'Номер біту (0–15)',
                type:     'number',
                required: true,
                min:      0,
                max:      15,
            },
            {
                key:      'regType',
                label:    'Тип регістру',
                type:     'select',
                required: true,
                options:  ['holding', 'input'],
            },
        ]
    },

};

// =====================================================
// Утиліти для роботи зі схемами
// =====================================================

/**
 * Генерує HTML поля форми для заданого decoding_type
 * @param {number} typeId — id типу (1, 2, 3...)
 * @param {object} currentSettings — поточні settings (для режиму редагування)
 * @returns {string} HTML рядок
 */
function buildSettingsFields(typeId, currentSettings = {}) {
    const schema = VALUE_SCHEMAS[typeId];
    if (!schema) return `<div class="form-group"><span style="color:var(--text-muted)">Невідомий тип декодування</span></div>`;

    return schema.fields.map(field => {
        const val = currentSettings[field.key] ?? field.default ?? '';

        if (field.type === 'select') {
            const options = field.options.map(opt =>
                `<option value="${opt}" ${val === opt ? 'selected' : ''}>${opt}</option>`
            ).join('');
            return `
                <div class="form-group">
                    <label class="form-label">${field.label}${field.required ? ' *' : ''}</label>
                    <select class="form-select" name="settings.${field.key}">${options}</select>
                </div>`;
        }

        if (field.type === 'checkbox') {
            return `
                <div class="form-group">
                    <div class="form-checkbox-row">
                        <input type="checkbox" id="settings-${field.key}" name="settings.${field.key}" ${val ? 'checked' : ''}>
                        <label for="settings-${field.key}">${field.label}</label>
                    </div>
                </div>`;
        }

        // number або text
        return `
            <div class="form-group">
                <label class="form-label">${field.label}${field.required ? ' *' : ''}</label>
                <input class="form-input" type="${field.type}" name="settings.${field.key}"
                    value="${val}"
                    ${field.min !== undefined ? `min="${field.min}"` : ''}
                    ${field.max !== undefined ? `max="${field.max}"` : ''}>
            </div>`;
    }).join('');
}

/**
 * Зчитує settings з форми і повертає об'єкт
 * @param {HTMLElement} formEl — контейнер форми
 * @returns {object}
 */
function readSettingsFromForm(formEl) {
    const settings = {};
    formEl.querySelectorAll('[name^="settings."]').forEach(el => {
        const key = el.name.replace('settings.', '');
        if (el.type === 'checkbox') {
            settings[key] = el.checked;
        } else if (el.type === 'number') {
            settings[key] = el.value !== '' ? Number(el.value) : null;
        } else {
            settings[key] = el.value;
        }
    });
    return settings;
}

/**
 * Повертає читабельну назву типу
 * @param {number} typeId
 * @returns {string}
 */
function getTypeName(typeId) {
    return VALUE_SCHEMAS[typeId]?.name ?? `Тип ${typeId}`;
}
